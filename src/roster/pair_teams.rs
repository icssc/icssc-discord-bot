use std::time::Duration;

use anyhow::{ensure, Context as _};
use entity::{social_team_pairing_entry, social_team_pairing_group, social_team_pairing_round};
use itertools::Itertools;
use migration::Expr;
use poise::CreateReply;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, FromQueryResult, QuerySelect, TransactionTrait};
use serenity::all::{
    ButtonStyle, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};

use crate::{
    AppContext, AppError, AppVars, matchy::{
        helpers::{Pairing, hash_seed},
        matching::graph_pair,
    }, util::base_embed
};

const TEAMS: [&str; 11] = [
    "AAA",
    "Corporate",
    "Events",
    "Graphics",
    "PR",
    // Projects
    "UI/UX",
    "AntAlmanac Scheduler",
    "AntAlmanac Planner",
    "Anteater API",
    "PeterPlate",
    "ZotMeet",
];

/// Get previously saved pairs
pub(crate) async fn get_previous_matches(data: &AppVars) -> Result<Vec<Vec<String>>, AppError> {
    #[derive(FromQueryResult)]
    struct Teams { teams: Vec<String> }

    let matches = social_team_pairing_entry::Entity::find()
        .select_only()
        .column_as(
            Expr::cust(r#"ARRAY_AGG(social_team_pairing_entry.team_name)"#),
            "teams",
        )
        .group_by(social_team_pairing_entry::Column::GroupId)
        .into_model::<Teams>()
        .all(&data.db)
        .await
        .context("fetch history from db")?
        .into_iter()
        .map(|row| row.teams)
        .collect_vec();

    Ok(matches)
}


/// Save new pairings to db
async fn save_pairs_to_db(ctx: AppContext<'_>, pairs: Vec<Vec<String>>) -> Result<(), AppError> {
    let round_sql = social_team_pairing_round::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
    };

    let conn = &ctx.data().db;
    conn.transaction(move |txn| Box::pin(async move {
        let round = round_sql.insert(txn).await.context("insert round")?;
        for teams in pairs {
                let group_sql = social_team_pairing_group::ActiveModel {
                    id: Default::default(),
                    round_id: ActiveValue::Set(round.id),
                };
                let group = group_sql.insert(txn).await.context("insert team pair")?;

                for team in teams {
                    let pair_member_sql = social_team_pairing_entry::ActiveModel {
                        group_id: ActiveValue::Set(group.id),
                        team_name: ActiveValue::Set(team.into()),
                    };
                    pair_member_sql
                        .insert(txn)
                        .await
                        .context("insert team")?;
                }
            }

        anyhow::Ok(())
    })).await?;

    Ok(())
}

/// Create team pairings for joint socials
#[poise::command(slash_command, guild_only)]
pub(crate) async fn pair_teams(
    ctx: AppContext<'_>,
    #[description = "Seed for the pairing, e.g. today's date"] seed: String,
) -> Result<(), AppError> {
    let seed = hash_seed(&seed);

    // :(
    let existing_pairs = get_previous_matches(ctx.data()).await?;
    let existing_pairs_ref = existing_pairs.iter()
        .map(|row| row.iter().map(String::as_str).collect_vec())
        .collect_vec();

    let Pairing(pairs, _) = graph_pair(TEAMS.to_vec(), &existing_pairs_ref, seed)?;

    let map_pair_to_str = |p: Vec<&str>| p.into_iter().map(String::from).collect_vec();
    let pairs = pairs.into_iter().map(map_pair_to_str).collect_vec();

    let content = pairs
        .iter()
        .map(|pair| format!("- {}", pair.join(" + ")))
        .join("\n");
    let footer_text = "Click save to track these pairings and prevent future repeats.";

    let embed = base_embed(ctx.serenity_context())
        .title("Created Team Pairings")
        .description(content)
        .footer(CreateEmbedFooter::new(footer_text));

    let save_button = CreateButton::new("teampair_save")
        .style(ButtonStyle::Primary)
        .label("Save");

    let components = vec![CreateActionRow::Buttons(vec![save_button])];

    let reply = ctx
        .send(
            CreateReply::default()
                .ephemeral(true)
                .embed(embed.clone())
                .components(components),
        )
        .await?;

    let reply_msg = reply.message().await?;
    match reply_msg
        .await_component_interaction(&ctx.serenity_context().shard)
        .timeout(Duration::from_mins(5))
        .await
    {
        Some(ixn) => {
            ensure!(
                matches!(ixn.data.kind, ComponentInteractionDataKind::Button)
                    && ixn.data.custom_id == "teampair_save",
                "unexpected component interaction"
            );
            let button = CreateButton::new("teampair_save")
                .style(ButtonStyle::Primary)
                .label("Saved")
                .disabled(true);
            let action_row = CreateActionRow::Buttons(vec![button]);

            save_pairs_to_db(ctx, pairs).await?;

            ixn.create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new().components(vec![action_row]),
                ),
            ).await?;
        }
        None => {
            let footer_text = "Interaction expired; re-run this command to save pairs.";
            let embed = embed.footer(CreateEmbedFooter::new(footer_text));
            reply
                .edit(ctx, CreateReply::default().embed(embed).components(vec![]))
                .await?;
        }
    };

    Ok(())
}
