use crate::spottings::util::opted_out_among;
use crate::util::message::get_members;
use crate::util::modal::ModalInputTexts;
use crate::util::paginate::{EmbedLinePaginator, PaginatorOptions};
use crate::util::text::comma_join;
use crate::util::{ContextExtras as _, spottings_embed};
use crate::{AppContext, AppError, AppVars};
use anyhow::{Context as _, bail};
use entity::{spotting_message, spotting_victim};
use itertools::Itertools as _;
use poise::{ChoiceParameter, CreateReply};
use sea_orm::{
    ActiveValue, ConnectionTrait as _, DbErr, EntityTrait as _, QueryOrder as _,
    TransactionTrait as _,
};
use sea_orm::{DatabaseConnection, TransactionError};
use serenity::all::{
    CacheHttp as _, CreateActionRow, CreateButton, CreateInputText, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateModal, GuildId, InputTextStyle, Mentionable as _,
    ModalInteraction, ReactionType, User, UserId,
};
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::str::FromStr as _;
use std::time::Duration;

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

/// Log a social or snipe
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn post(
    ctx: AppContext<'_>,
    #[description = "Link to message with proof"] message: serenity::all::Message,
    #[description = "Was this a social or a snipe?"] r#type: SpottingType,
    #[description = "The first victim"] victim1: User,
    #[description = "Another victim, if applicable"] victim2: Option<User>,
    #[description = "Another victim, if applicable"] victim3: Option<User>,
    #[description = "Another victim, if applicable"] victim4: Option<User>,
    #[description = "Another victim, if applicable"] victim5: Option<User>,
    #[description = "Another victim, if applicable"] victim6: Option<User>,
    #[description = "Another victim, if applicable"] victim7: Option<User>,
    #[description = "Another victim, if applicable"] victim8: Option<User>,
    // #[description = "Another victim, if applicable"] victim9: Option<User>,
    // #[description = "Another victim, if applicable"] victim10: Option<User>,
) -> Result<(), AppError> {
    let victims = vec![
        Some(victim1),
        victim2,
        victim3,
        victim4,
        victim5,
        victim6,
        victim7,
        victim8,
        // victim9,
        // victim10,
    ]
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();

    if victims.iter().any(|v| v.bot) {
        ctx.reply_ephemeral("sanity check: bots don't have physical forms to snipe!")
            .await?;
        return Ok(());
    }

    // if message.guild_id != ctx.guild_id() {
    //     ctx.reply("that message isn't in this guild...").await?;
    //     return Ok(());
    // }

    if message
        .attachments
        .iter()
        .all(|attachment| attachment.height.is_none())
    {
        ctx.reply_ephemeral("no images in your linked message!")
            .await?;
        return Ok(());
    }

    let conn = &ctx.data().db;

    if matches!(r#type, SpottingType::Snipe)
        && let opted_out = opted_out_among(conn, victims.iter().map(|u| u.id))
            .await?
            .collect_vec()
        && !opted_out.is_empty()
    {
        ctx.send(CreateReply::default().embed(spottings_embed().description(format!(
            "**the following people in that post are opted out of sniping!**\n{}\n\nthis means they do not consent to being photographed!",
            opted_out.into_iter().map(|uid| uid.mention()).join("\n"),
        ))).reply(true).ephemeral(true)).await?;
        return Ok(());
    }

    let emb = spottings_embed().description(format!(
        "**you are claiming that {} spotted**:\n{}\n\nclick to confirm! (times out in 15 seconds)",
        message.author.mention(),
        victims.iter().join("")
    ));

    let post_confirm_id = "spotting_post_confirm";

    let handle = ctx
        .send(
            CreateReply::default()
                .embed(emb.clone())
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new(post_confirm_id)
                        .emoji(ReactionType::Unicode(String::from("😎"))),
                ])])
                .reply(true)
                .ephemeral(true),
        )
        .await?;

    let Some(waited) = handle
        .message()
        .await?
        .await_component_interaction(&ctx.serenity_context().shard)
        .author_id(ctx.author().id)
        .custom_ids(vec![String::from(post_confirm_id)])
        .timeout(Duration::from_secs(15))
        .await
    else {
        ctx.reply_ephemeral("ok, nevermind then").await?;
        return Ok(());
    };

    let victims = victims.into_iter().map(|user| user.id).collect_vec();

    let Ok(_) = add_spottings_to_db(conn, r#type, ctx.guild_id().unwrap(), &message, victims).await
    else {
        ctx.reply_ephemeral("couldn't insert :(").await?;
        return Ok(());
    };

    // remove "please react below..." and button
    waited
        .create_response(
            ctx.http(),
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(emb.clone())
                    .components(vec![]),
            ),
        )
        .await?;

    let _ = message
        .react(ctx.http(), ReactionType::Unicode("👏".to_string()))
        .await;

    ctx.reply_ephemeral("ok, logged").await?;
    Ok(())
}

// #[derive(FromQueryResult)]
// struct ImplodedSnipes {
//     guild_id: i64,
//     channel_id: i64,
//     message_id: i64,
//     author_id: i64,
//     time_posted: DateTime,
//     first_name: String,
//     last_name: String,
//     victims: Vec<i64>,
// }

/// Log past snipes
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn history(ctx: AppContext<'_>) -> Result<(), AppError> {
    let conn = &ctx.data().db;

    let got = spotting_message::Entity::find()
        // .column_as(Expr::cust("array_agg(snipe.victim_id)"), "victims")
        .find_with_related(spotting_victim::Entity)
        // .group_by(message::Column::MessageId)
        .order_by_desc(spotting_message::Column::MessageId)
        // .into_model::<ImplodedSnipes>()
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
