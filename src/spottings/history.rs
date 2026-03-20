use std::num::NonZeroUsize;

use anyhow::{Context as _, bail, ensure};
use entity::{spotting_message, spotting_victim};
use itertools::Itertools as _;
use poise::ChoiceParameter;
use sea_orm::{ColumnTrait as _, Condition, EntityTrait as _, QueryFilter as _, QueryOrder as _};
use serenity::all::{Mentionable as _, User, UserId};

use crate::{
    AppContext, AppError,
    util::{
        paginate::{EmbedLinePaginator, PaginatorOptions},
        text::comma_join,
    },
};

#[derive(ChoiceParameter, PartialEq, Eq, Copy, Clone, Debug, Hash)]
enum HistoryFilterType {
    #[name = "Socials"]
    Social,
    #[name = "Snipes"]
    Snipe,
}

/// View the history of past snipes
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn history(
    ctx: AppContext<'_>,
    #[description = "Type of spotting to filter by"] mut r#type: Option<HistoryFilterType>,
    #[description = "User who is present in either a snipe or social"] participant: Option<User>,
    #[description = "(snipes only) the person taking the snipe"] sniper: Option<User>,
    #[description = "(snipes only) the person being sniped"] snipe_victim: Option<User>,
) -> Result<(), AppError> {
    let has_snipe_field = sniper.is_some() || snipe_victim.is_some();

    if has_snipe_field {
        ensure!(
            participant.is_none(),
            "`participant` cannot be used with `sniper` or `snipe_victim`"
        );
        // automatically set the filter type if possible
        r#type = match r#type {
            Some(HistoryFilterType::Social) => {
                bail!("`sniper` or `snipe_victim` is for snipes only")
            }
            _ => Some(HistoryFilterType::Snipe),
        };
    }

    let conn = &ctx.data().db;

    let filter_cond = [
        r#type.map(|t: HistoryFilterType| {
            Condition::all()
                .add(spotting_message::Column::IsSocial.eq(t == HistoryFilterType::Social))
        }),
        participant.map(|user| {
            Condition::any()
                .add(spotting_message::Column::AuthorId.eq(user.id.get()))
                .add(spotting_victim::Column::VictimId.eq(user.id.get()))
        }),
        sniper
            .map(|user| Condition::all().add(spotting_message::Column::AuthorId.eq(user.id.get()))),
        snipe_victim
            .map(|user| Condition::all().add(spotting_victim::Column::VictimId.eq(user.id.get()))),
    ]
    .into_iter()
    .flatten()
    .fold(Condition::all(), Condition::add);

    let got = spotting_message::Entity::find()
        .order_by_desc(spotting_message::Column::MessageId)
        .find_with_related(spotting_victim::Entity)
        .filter(filter_cond)
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
