use crate::spottings::{
    history::history,
    leaderboard::leaderboard,
    privacy::{check_snipes_participation, set_snipes_participation},
};
use crate::{AppContext, AppError};

#[poise::command(
    prefix_command,
    slash_command,
    subcommands(
        "leaderboard",
        "history",
        "check_snipes_participation",
        "set_snipes_participation",
    ),
    guild_only
)]
pub(crate) async fn spottings(ctx: AppContext<'_>) -> Result<(), AppError> {
    ctx.reply("base command is a noop").await?;
    Ok(())
}
