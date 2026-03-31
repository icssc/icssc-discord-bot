use std::time::Duration;

use anyhow::ensure;
use itertools::Itertools;
use poise::CreateReply;
use serenity::all::{
    ButtonStyle, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};

use crate::{
    AppContext, AppError,
    matchy::{
        helpers::{Pairing, hash_seed},
        matching::graph_pair,
    },
    util::base_embed,
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

async fn save_pairs_to_db(pairs: Vec<Vec<&str>>) -> Result<(), AppError> {
    // do stuff

    Ok(())
}

/// Create team pairings for joint socials
#[poise::command(slash_command, guild_only)]
pub(crate) async fn pair_teams(
    ctx: AppContext<'_>,
    #[description = "Seed for the pairing, e.g. today's date"] seed: String,
) -> Result<(), AppError> {
    let seed = hash_seed(&seed);
    let Pairing(pairs, _) = graph_pair(TEAMS.to_vec(), &[], seed)?;

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

            save_pairs_to_db(pairs).await?;

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
