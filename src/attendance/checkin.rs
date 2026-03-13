use std::{collections::HashSet, str::FromStr as _};

use anyhow::{Context as _, Error, bail};
use itertools::Itertools as _;
use serenity::{
    all::{
        CacheHttp as _, CreateActionRow, CreateInputText, CreateInteractionResponse, CreateModal,
        EditInteractionResponse, InputTextStyle, ModalInteraction, ReactionType, UserId,
    },
    futures::future,
};

use crate::{
    AppContext, AppError, AppVars,
    util::{
        ContextExtras as _,
        message::get_members,
        modal::ModalInputTexts,
        roster::{check_in_with_email, get_bulk_members_from_roster, get_user_from_discord},
    },
};

/// Check into today's ICSSC event!
#[poise::command(slash_command, hide_in_help)]
pub(crate) async fn checkin(ctx: AppContext<'_>) -> Result<(), Error> {
    let Ok(_) = ctx
        .data()
        .google_service_account
        .write()
        .await
        .get_access_token("https://www.googleapis.com/auth/spreadsheets.readonly")
        .await
    else {
        ctx.reply_ephemeral("Unable to find who you are :(").await?;
        return Ok(());
    };

    ctx.defer_ephemeral().await?;

    let username = &ctx.author().name;
    let Ok(Some(user)) = get_user_from_discord(ctx.data(), username.clone()).await else {
        ctx.reply_ephemeral(
            "\
Cannot find a matching internal member. Double check that your \
Discord username on the internal roster is correct.",
        )
        .await?;
        return Ok(());
    };

    let Ok(_) = check_in_with_email(ctx.data(), &user.email, None).await else {
        ctx.reply_ephemeral("Unable to check in").await?;
        return Ok(());
    };

    ctx.reply_ephemeral(format!("Successfully checked in as {}", user.name))
        .await?;
    Ok(())
}

/// Count a message as attendance for an ICSSC event
#[poise::command(context_menu_command = "Log Attendance", guild_only)]
pub(crate) async fn log_attendance(
    ctx: AppContext<'_>,
    message: serenity::all::Message,
) -> Result<(), Error> {
    let AppContext::Application(ctx) = ctx else {
        bail!("unexpected context type")
    };

    let data = ctx.data();
    let is_matchy_channel = data.channels.matchy_channel_id == message.channel_id.get();
    let default_event_name = match is_matchy_channel {
        true => "Matchy Meetup",
        false => "",
    };

    let members: HashSet<String> = get_members(&message, true);

    // create inputs
    let msg_input = CreateActionRow::InputText(
        CreateInputText::new(InputTextStyle::Short, "Message ID", "message_id")
            .value(message.id.to_string())
            .required(true),
    );
    let event_name_input = CreateActionRow::InputText(
        CreateInputText::new(InputTextStyle::Short, "Name of Event", "event_name")
            .value(default_event_name)
            .required(false),
    );
    let members_input = CreateActionRow::InputText(
        CreateInputText::new(
            InputTextStyle::Paragraph,
            "Who was at this event?",
            "participants",
        )
        .value(members.iter().join("\n"))
        .required(true),
    );

    let modal = CreateModal::new("attendance_log_modal_confirm", "Confirm Attendance")
        .components(vec![msg_input, event_name_input, members_input]);

    let reply = CreateInteractionResponse::Modal(modal);
    ctx.interaction.create_response(ctx.http(), reply).await?;

    Ok(())
}

pub(crate) async fn confirm_attendance_log_modal(
    ctx: &serenity::all::Context,
    data: &'_ AppVars,
    ixn: &ModalInteraction,
) -> Result<(), AppError> {
    let inputs = ModalInputTexts::new(ixn);
    let message = inputs
        .get_required_value("message_id")?
        .parse::<u64>()
        .context("unexpected non-numerical message ID")
        .map(|id| ixn.channel_id.message(ctx.http(), id))?
        .await?;

    let attendees = inputs.get_required_value("participants")?;
    let event_name = inputs.get_value("event_name")?;

    ixn.defer_ephemeral(ctx.http()).await?;

    let participant_ids = attendees.split('\n');
    let participants = future::join_all(participant_ids.clone().filter_map(|s| {
        let uid = UserId::from_str(s.trim()).ok()?;
        Some(ixn.guild_id?.member(ctx.http(), uid))
    }))
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .context("Some user IDs not found")?;

    let usernames = participants
        .into_iter()
        .map(|member| member.user.name)
        .collect_vec();

    let members = get_bulk_members_from_roster(data, &usernames).await?;
    let is_missing = members.len() != usernames.len();
    if is_missing {
        bail!("user lookup failed");
    }

    let mut response_lines = Vec::new();
    for member in members {
        let success = check_in_with_email(data, &member.email, event_name.as_deref())
            .await
            .is_ok();
        let emoji = match success {
            true => "☑️",
            false => "❌",
        };
        let line = format!("{} {} ({})", emoji, member.name, member.email);
        response_lines.push(line);
    }

    let content = String::from("Submitted attendance for the following users:\n")
        + &response_lines.join("\n");

    ixn.edit_response(ctx.http(), EditInteractionResponse::new().content(content))
        .await?;

    let _ = message
        .react(ctx.http(), ReactionType::Unicode("👋".to_owned()))
        .await;

    Ok(())
}
