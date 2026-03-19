use std::num::NonZeroUsize;

use anyhow::Context as _;
use entity::{spotting_message, spotting_victim};
use itertools::Itertools as _;
use sea_orm::{EntityTrait as _, QueryOrder as _};
use serenity::all::{Mentionable as _, UserId};

use crate::{
    AppContext, AppError,
    util::{
        paginate::{EmbedLinePaginator, PaginatorOptions},
        text::comma_join,
    },
};

/// View the history of past snipes
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn history(ctx: AppContext<'_>) -> Result<(), AppError> {
    let conn = &ctx.data().db;

    let got = spotting_message::Entity::find()
        .order_by_desc(spotting_message::Column::MessageId)
        .find_with_related(spotting_victim::Entity)
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
