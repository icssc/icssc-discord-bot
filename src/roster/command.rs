use crate::AppContext;
use crate::AppError;
use crate::roster::desynced::check_discord_roles;
use crate::roster::desynced::check_google_access;
use crate::roster::pair_teams::pair_teams;
use crate::roster::user_lookup::lookup_discord;

#[poise::command(
    prefix_command,
    slash_command,
    subcommands(
        "check_discord_roles",
        "check_google_access",
        "lookup_discord",
        "pair_teams"
    ),
    guild_only
)]
pub(crate) async fn roster(ctx: AppContext<'_>) -> Result<(), AppError> {
    ctx.reply("base command is a noop").await?;
    Ok(())
}
