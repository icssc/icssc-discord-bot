use entity::spotting_message;
use sea_orm::EntityTrait as _;
use serenity::all::{Message, ReactionType};

use crate::{AppContext, AppError, util::ContextExtras as _};

#[poise::command(context_menu_command = "Remove Spotting", guild_only)]
pub(crate) async fn remove_spotting(ctx: AppContext<'_>, message: Message) -> Result<(), AppError> {
    let msg_id = message.id.get() as i64;
    let delete_ct = spotting_message::Entity::delete_by_id(msg_id)
        .exec(&ctx.data().db)
        .await?
        .rows_affected;

    if delete_ct == 0 {
        ctx.reply_ephemeral(
            "Error: this message is not logged as a snipe (👏) or social (🙌). \
        Note that events that count for attendance (👋) cannot be un-logged with this command.",
        )
        .await?;
        return Ok(());
    }

    ctx.reply_ephemeral("Removed message from spotting logs")
        .await?;
    let http = ctx.http();
    let reaction_del_user = Some(ctx.cache().current_user().id);
    message
        .delete_reaction(
            http,
            reaction_del_user,
            ReactionType::Unicode("👏".to_owned()),
        )
        .await?;
    message
        .delete_reaction(
            http,
            reaction_del_user,
            ReactionType::Unicode("🙌".to_owned()),
        )
        .await?;

    Ok(())
}
