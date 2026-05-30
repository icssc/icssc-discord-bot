use crate::util::paginate::{EmbedLinePaginator, PaginatorOptions};
use crate::{AppContext, AppError};
use anyhow::{Context as _, anyhow};
use entity::user_stat;
use itertools::Itertools as _;
use migration::NullOrdering;
use pluralizer::pluralize;
use poise::{ChoiceParameter, CreateReply};
use sea_orm::sea_query::{Expr, Func};
use sea_orm::{
    ColumnTrait as _, Condition, EntityTrait as _, FromQueryResult, Order, QueryFilter as _,
    QueryOrder as _, QuerySelect as _,
};
use serenity::all::{CreateEmbed, Mentionable as _, UserId};
use std::num::NonZeroUsize;

#[derive(ChoiceParameter, PartialEq, Eq, Copy, Clone, Debug, Hash)]
enum LeaderboardBy {
    #[name = "Total points"]
    TotalPoints,
    #[name = "Number of socials"]
    SocialCount,
    #[name = "Total snipes"]
    SnipeCount,
    #[name = "Times sniped"]
    VictimCount,
    #[name = "Ratio of total snipes to times sniped"]
    SnipeRate,
}

async fn show_summary_leaderboard(ctx: AppContext<'_>) -> anyhow::Result<()> {
    let conn = &ctx.data().db;

    let top5_overall = user_stat::Entity::find()
        .order_by_desc(
            Expr::col(user_stat::Column::SnipesInitiated)
                .add(Expr::col(user_stat::Column::SocialsInitiated).mul(2))
                .add(Expr::col(user_stat::Column::SocialsVictim).mul(2)),
        )
        .limit(5)
        .all(conn)
        .await
        .context("fetch top 5 from db")?
        .into_iter()
        .map(|row| {
            let social_ct = row.socials_initiated + row.socials_victim;
            let total = row.snipes_initiated + social_ct * 2;
            let snipes_text = pluralize("snipe", row.snipes_initiated as isize, true);
            let socials_text = pluralize("social", social_ct as isize, true);
            format!(
                "1. <@{}>: {total} points ({snipes_text} + {socials_text})",
                row.id
            )
        })
        .join("\n");

    let top_sniper = user_stat::Entity::find()
        .order_by_desc(user_stat::Column::SnipesInitiated)
        .limit(1)
        .one(conn)
        .await
        .context("fetch top sniper")?
        .map(|row| {
            format!(
                "🔭 **Most Snipes:** <@{}> ({})",
                row.id, row.snipes_initiated
            )
        })
        .ok_or(anyhow!("missing top sniper"))?;

    let top_social = user_stat::Entity::find()
        .order_by_desc(
            Expr::col(user_stat::Column::SocialsInitiated)
                .add(Expr::col(user_stat::Column::SocialsVictim)),
        )
        .limit(1)
        .one(conn)
        .await
        .context("fetch top social")?
        .map(|row| {
            let social_ct = row.socials_initiated + row.socials_victim;
            format!("😋 **Most Socials:** <@{}> ({})", row.id, social_ct)
        })
        .ok_or(anyhow!("missing top sniper"))?;

    let embed = CreateEmbed::new()
        .color(0xc0d9e5)
        .title("ICSSC Spottings Leaderboard")
        .thumbnail("https://cdn.discordapp.com/avatars/1336510972403126292/8db135d66c041c0191e0ae8085b9baa6.webp?size=512")
        .description(
            format!("This is the overall spottings leaderboard. **Snipes** are worth 1 point per person \
                sniped, and **socials** are worth 2 points each.\n\n\
                **Top Overall Scores:**\n\n{top5_overall}\n\n\
                {top_sniper}\n\
                {top_social}\n\n\
                -# To view a specific type of leaderboard, provide a value to the optional `by` \
                parameter in `/spottings leaderboard by:type`")
        );

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;

    Ok(())
}

#[derive(FromQueryResult)]
struct SnipeRateQuery {
    id: i64,
    snipe_rate: Option<f64>,
}

/// Show leaderboards by various sniping statistics
#[poise::command(prefix_command, slash_command, guild_only)]
pub(crate) async fn leaderboard(
    ctx: AppContext<'_>,
    #[description = "The type of leaderboard to show"] by: Option<LeaderboardBy>,
) -> Result<(), AppError> {
    let Some(by) = by else {
        show_summary_leaderboard(ctx).await?;
        return Ok(());
    };

    let lines = match by {
        LeaderboardBy::TotalPoints => user_stat::Entity::find()
            .order_by_desc(
                Expr::col(user_stat::Column::SnipesInitiated)
                    .add(Expr::col(user_stat::Column::SocialsInitiated).mul(2))
                    .add(Expr::col(user_stat::Column::SocialsVictim).mul(2)),
            )
            .filter(
                Condition::any()
                    .add(user_stat::Column::SocialsInitiated.ne(0))
                    .add(user_stat::Column::SocialsVictim.ne(0))
                    .add(user_stat::Column::SnipesInitiated.ne(0)),
            )
            .all(&ctx.data().db)
            .await
            .context("fetch leaderboard from db")?
            .into_iter()
            .map(|mdl| {
                let social_ct = mdl.socials_initiated + mdl.socials_victim;
                let total = mdl.snipes_initiated + social_ct * 2;
                let snipes_text = pluralize("snipe", mdl.snipes_initiated as isize, true);
                let socials_text = pluralize("social", social_ct as isize, true);
                format!(
                    "1. <@{}>: {total} points ({snipes_text} + {socials_text})",
                    mdl.id
                )
                .into_boxed_str()
            })
            .collect_vec(),
        LeaderboardBy::SocialCount => user_stat::Entity::find()
            .order_by_desc(
                Expr::col(user_stat::Column::SocialsInitiated)
                    .add(Expr::col(user_stat::Column::SocialsVictim)),
            )
            .all(&ctx.data().db)
            .await
            .context("fetch leaderboard from db")?
            .into_iter()
            .map(|mdl| {
                format!(
                    "1. {}: {}",
                    UserId::from(mdl.id as u64).mention(),
                    mdl.socials_initiated + mdl.socials_victim
                )
                .into_boxed_str()
            })
            .collect_vec(),
        LeaderboardBy::SnipeCount => user_stat::Entity::find()
            .order_by_desc(user_stat::Column::SnipesInitiated)
            .all(&ctx.data().db)
            .await
            .context("fetch leaderboard from db")?
            .into_iter()
            .enumerate()
            .map(|(i, mdl)| {
                format!(
                    "{}. {}: {}",
                    i + 1,
                    UserId::from(mdl.id as u64).mention(),
                    mdl.snipes_initiated
                )
                .into_boxed_str()
            })
            .collect_vec(),
        LeaderboardBy::VictimCount => user_stat::Entity::find()
            .order_by_desc(user_stat::Column::SnipesVictim)
            .all(&ctx.data().db)
            .await
            .context("fetch leaderboard from db")?
            .into_iter()
            .enumerate()
            .map(|(i, mdl)| {
                format!(
                    "{}. {}: {}",
                    i + 1,
                    UserId::from(mdl.id as u64).mention(),
                    mdl.snipes_victim
                )
                .into_boxed_str()
            })
            .collect_vec(),
        LeaderboardBy::SnipeRate => user_stat::Entity::find()
            .select_only()
            .column(user_stat::Column::Id)
            .column_as(
                Expr::col(user_stat::Column::SnipesInitiated)
                    .cast_as("double precision")
                    .div(
                        Func::cust("NULLIF")
                            .arg(
                                Expr::col(user_stat::Column::SnipesVictim)
                                    .cast_as("double precision"),
                            )
                            .arg(0),
                    ),
                "snipe_rate",
            )
            .filter(
                Condition::any()
                    .add(user_stat::Column::SnipesInitiated.ne(0))
                    .add(user_stat::Column::SnipesVictim.ne(0)),
            )
            .order_by_with_nulls(Expr::col("snipe_rate"), Order::Desc, NullOrdering::First)
            .order_by_desc(user_stat::Column::SnipesInitiated)
            .into_model::<SnipeRateQuery>()
            .all(&ctx.data().db)
            .await
            .context("fetch leaderboard from db")?
            .into_iter()
            .enumerate()
            .map(|(i, mdl)| {
                format!(
                    "{}. {}: {}",
                    i + 1,
                    UserId::from(mdl.id as u64).mention(),
                    mdl.snipe_rate
                        .map_or(String::from("\u{2013}"), |n| n.to_string())
                )
                .into_boxed_str()
            })
            .collect_vec(),
    };

    let paginator = EmbedLinePaginator::new(
        lines,
        PaginatorOptions::default()
            .sep("\n".into())
            .max_lines(NonZeroUsize::new(10).unwrap())
            .ephemeral(true),
    );

    paginator.run(ctx).await.context("start paginator")?;
    Ok(())
}
