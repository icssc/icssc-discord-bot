use anyhow::bail;
use poise::CreateReply;
use serenity::all::{
    CreateEmbed, CreateEmbedAuthor, Member, Mentionable as _, User,
};

use crate::{
    AppContext, AppError, AppVars,
    attendance::attended::get_events_attended_text,
    matchy::opt_in::MatchyMeetupOptIn,
    spottings::{privacy::SnipesOptOut, socials_role::SocialsParticipation},
    util::{
        ContextExtras as _, base_embed,
        roster::{RosterSheetRow, get_user_from_discord},
    },
};

#[poise::command(context_menu_command = "Lookup Member", guild_only)]
pub(crate) async fn user_lookup(ctx: AppContext<'_>, user: User) -> Result<(), AppError> {
    let Some(guild_id) = ctx.guild_id() else {
        bail!("command not executed in guild");
    };

    let row = get_user_from_discord(ctx.data(), None, user.name).await?;
    let Some(row) = row else {
        ctx.reply_ephemeral("User is not an internal member")
            .await?;
        return Ok(());
    };

    let http = ctx.http();
    let member = http.get_member(guild_id, user.id).await?;

    ctx.defer_ephemeral().await?;

    let response_embed =
        lookup_result_embed(ctx.serenity_context(), ctx.data(), &member, row).await?;

    ctx.send(CreateReply::default().embed(response_embed).ephemeral(true))
        .await?;

    Ok(())
}

fn participation_field_text(opted_in: bool) -> &'static str {
    match opted_in {
        true => "Opted In \u{2705}",
        false => "Opted Out \u{274c}",
    }
}

async fn lookup_result_embed(
    ctx: &serenity::all::Context,
    data: &AppVars,
    guild_member: &Member,
    roster_row: RosterSheetRow,
) -> Result<CreateEmbed, AppError> {
    let user = &guild_member.user;
    let RosterSheetRow { name, email, .. } = roster_row;

    let user_lines = format!("Discord: {}\nEmail: {email}", user.mention());
    let events_lines = get_events_attended_text(data, None, &email).await?;
    let events_header = match events_lines.len() {
        0 => "No events attended :(".to_owned(),
        ct => format!("## Events Attended ({ct})"),
    };
    let joined_matchy = MatchyMeetupOptIn::new(ctx, data)
        .contains_user(user.id)
        .await?;
    let joined_socials = SocialsParticipation::new(ctx, data)
        .has_role(guild_member)
        .await?;
    let left_snipes = SnipesOptOut::new(ctx, data).contains_user(user.id).await?;

    let avatar_url = user.avatar_url().unwrap_or(user.default_avatar_url());
    let embed = base_embed(ctx)
        .author(CreateEmbedAuthor::new(name).icon_url(avatar_url))
        .field("Matchy", participation_field_text(joined_matchy), true)
        .field("Socials", participation_field_text(joined_socials), true)
        .field("Snipes", participation_field_text(!left_snipes), true)
        .description(format!(
            "{user_lines}\n{events_header}\n{}",
            events_lines.join("\n")
        ));

    Ok(embed)
}

/// Shows internal member information for the specified user
#[poise::command(slash_command, hide_in_help, guild_only)]
pub(crate) async fn lookup_discord(ctx: AppContext<'_>, user: User) -> Result<(), AppError> {
    let Some(guild_id) = ctx.guild_id() else {
        bail!("command not executed in guild");
    };

    let row = get_user_from_discord(ctx.data(), None, user.name.clone()).await?;
    let Some(row) = row else {
        ctx.reply_ephemeral("User is not an internal member")
            .await?;
        return Ok(());
    };

    let http = ctx.http();
    let Ok(member) = http.get_member(guild_id, user.id).await else {
        ctx.reply_ephemeral("User is not a server member").await?;
        return Ok(());
    };

    ctx.defer_ephemeral().await?;

    let response_embed =
        lookup_result_embed(ctx.serenity_context(), ctx.data(), &member, row).await?;

    ctx.send(CreateReply::default().embed(response_embed).ephemeral(true))
        .await?;

    Ok(())
}
