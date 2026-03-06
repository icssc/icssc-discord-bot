use crate::spottings::util::opted_out_among;
use crate::util::message::get_members;
use crate::util::modal::ModalInputTexts;
use crate::util::paginate::{EmbedLinePaginator, PaginatorOptions};
use crate::util::text::comma_join;
use crate::{AppContext, AppError, AppVars};
use anyhow::{Context as _, bail};
use entity::{spotting_message, spotting_victim};
use itertools::Itertools as _;
use poise::ChoiceParameter;
use sea_orm::{
    ActiveValue, ConnectionTrait as _, DbErr, EntityTrait as _, QueryOrder as _,
    TransactionTrait as _,
};
use sea_orm::{DatabaseConnection, TransactionError};
use serenity::all::{
    CacheHttp as _, CreateActionRow, CreateInputText, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateModal, GuildId, InputTextStyle, Mentionable as _,
    ModalInteraction, ReactionType, UserId,
};
use std::num::NonZeroUsize;
use std::str::FromStr as _;

#[derive(PartialEq, Eq, ChoiceParameter)]
enum SpottingType {
    Social,
    Snipe,
}

async fn add_spottings_to_db(
    conn: &DatabaseConnection,
    r#type: SpottingType,
    guild_id: GuildId,
    message: &serenity::all::Message,
    victims: impl IntoIterator<Item = UserId>,
) -> Result<(), TransactionError<sea_orm::DbErr>> {
    let message_sql = spotting_message::ActiveModel {
        // command is guild_only
        guild_id: ActiveValue::Set(guild_id.into()),
        channel_id: ActiveValue::Set(message.channel_id.into()),
        message_id: ActiveValue::Set(message.id.into()),
        author_id: ActiveValue::Set(message.author.id.into()),
        time_posted: ActiveValue::NotSet,
        is_social: ActiveValue::Set(match r#type {
            SpottingType::Social => true,
            SpottingType::Snipe => false,
        }),
    };

    let snipes_sql = victims
        .into_iter()
        .map(|victim| spotting_victim::ActiveModel {
            message_id: ActiveValue::Set(message.id.into()),
            victim_id: ActiveValue::Set(victim.into()),
            latitude: ActiveValue::Set(None),
            longitude: ActiveValue::Set(None),
            notes: ActiveValue::Set(None),
        })
        .collect_vec();

    conn.transaction::<_, (), DbErr>(move |txn| {
        Box::pin(async move {
            spotting_message::Entity::insert(message_sql)
                .on_conflict_do_nothing()
                .exec(txn)
                .await?;
            spotting_victim::Entity::insert_many(snipes_sql)
                .on_conflict_do_nothing()
                .exec(txn)
                .await?;

            txn.execute_unprepared("REFRESH MATERIALIZED VIEW user_stat")
                .await?;

            Ok(())
        })
    })
    .await?;

    Ok(())
}

#[poise::command(context_menu_command = "Log Spotting", guild_only)]
pub(crate) async fn log_message_spotting(
    ctx: AppContext<'_>,
    message: serenity::all::Message,
) -> Result<(), AppError> {
    let spotted = get_members(&message, false);

    let AppContext::Application(ctx) = ctx else {
        bail!("unexpected context type");
    };

    // TODO update when labels are supported
    // let spotter_input = CreateActionRow::InputText(
    //     CreateSelectMenu::new("spotting_modal_spotter", CreateSelectMenuKind::User {
    //         default_users: Some(vec![message.author.id])
    //     })
    // );
    // let spotted_input = CreateActionRow::SelectMenu(
    //     CreateSelectMenu::new("spotting_modal_spotted", CreateSelectMenuKind::User {
    //         default_users: Some(spotted)
    //     })
    // );
    let msg_input = CreateActionRow::InputText(
        CreateInputText::new(InputTextStyle::Short, "Message ID", "spotting_modal_msg")
            .value(message.id.to_string())
            .required(true),
    );

    let spotted_input = CreateActionRow::InputText(
        CreateInputText::new(
            InputTextStyle::Paragraph,
            "Who was spotted?",
            "spotting_modal_spotted",
        )
        .value(spotted.iter().join("\n"))
        .required(true),
    );

    let spotting_type_input = CreateActionRow::InputText(
        CreateInputText::new(
            InputTextStyle::Short,
            "Type of Spotting (snipe | social)",
            "spotting_type",
        )
        .value("snipe")
        .required(true),
    );

    let modal = CreateModal::new("spotting_modal_confirm", "Confirm Spotting").components(vec![
        msg_input,
        spotted_input,
        spotting_type_input,
    ]);

    let reply = CreateInteractionResponse::Modal(modal);

    ctx.interaction.create_response(ctx.http(), reply).await?;

    Ok(())
}

pub(crate) async fn confirm_message_spotting_modal(
    ctx: &serenity::all::Context,
    data: &'_ AppVars,
    ixn: &ModalInteraction,
) -> Result<(), AppError> {
    let inputs = ModalInputTexts::new(ixn);
    let message = inputs
        .get_required_value("spotting_modal_msg")?
        .parse::<u64>()
        .context("unexpected non-numerical message ID")
        .map(|id| ixn.channel_id.message(ctx.http(), id))?
        .await?;

    if message
        .attachments
        .iter()
        .all(|attachment| attachment.height.is_none())
    {
        bail!("No images in your linked message!");
    }

    let spotted_uids = inputs
        .get_required_value("spotting_modal_spotted")?
        .split('\n')
        .filter_map(|s| {
            // TODO validate that user ids are actually in the server
            UserId::from_str(s.trim()).ok()
        })
        .collect_vec();

    // TODO components v2 dropdown
    let spotting_type = match inputs.get_required_value("spotting_type")?.as_str() {
        "snipe" => SpottingType::Snipe,
        "social" => SpottingType::Social,
        _ => bail!("unexpected spotting type"),
    };

    if spotting_type == SpottingType::Snipe
        && let opted_out = opted_out_among(
            &data.db,
            std::iter::once(message.author.id).chain(spotted_uids.iter().copied()),
        )
        .await?
        .collect_vec()
        && !opted_out.is_empty()
    {
        bail!(format!(
            "Can't proceed, the following users are opted out:\n{}",
            opted_out.into_iter().map(|uid| uid.mention()).join("\n")
        ));
    }

    let reaction = ReactionType::Unicode(
        match spotting_type {
            SpottingType::Snipe => "👏",
            SpottingType::Social => "🙌",
        }
        .to_owned(),
    );

    // write snipe to db
    let response = match add_spottings_to_db(
        &data.db,
        spotting_type,
        ixn.guild_id.unwrap(),
        &message,
        spotted_uids,
    )
    .await
    {
        Ok(_) => "ok, logged",
        _ => "couldn't insert :(",
    };

    ixn.create_response(
        ctx.http(),
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(response)
                .ephemeral(true),
        ),
    )
    .await?;

    let _ = message.react(ctx.http(), reaction).await;

    Ok(())
}

/// View the history of past snipes
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn history(ctx: AppContext<'_>) -> Result<(), AppError> {
    let conn = &ctx.data().db;

    let got = spotting_message::Entity::find()
        .find_with_related(spotting_victim::Entity)
        .order_by_desc(spotting_message::Column::MessageId)
        .all(conn)
        .await
        .context("log get recent snipes")?;

    let paginator = EmbedLinePaginator::new(
        got.iter()
            .map(|(msg, victims)| {
                format!(
                    "<t:{}:d> at <t:{0}:t>: **{}** spotted {} ([original {}](https://discord.com/channels/{}/{}/{}))",
                    msg.time_posted.and_utc().timestamp(),
                    UserId::from(msg.author_id as u64).mention(),
                    comma_join(
                        victims
                            .iter()
                            .map(|victim| UserId::from(victim.victim_id as u64).mention())
                    ),
                    if msg.is_social { "social" } else { "snipe" },
                    msg.guild_id,
                    msg.channel_id,
                    msg.message_id,
                )
                .into_boxed_str()
            })
            .collect_vec(),
        PaginatorOptions::default()
            .sep("\n\n".into())
            .max_lines(NonZeroUsize::new(10).unwrap())
            .ephemeral(true),
    );

    paginator.run(ctx).await.context("log paginate")?;

    Ok(())
}
