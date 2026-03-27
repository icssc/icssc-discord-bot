use super::helpers::Pairing;
use super::matching::graph_pair;
use crate::AppContext;
use crate::matchy::participation::{get_current_opted_in, get_previous_matches};
use anyhow::{Result, bail};
use itertools::Itertools as _;
use serenity::all::{PartialGuild, RoleId, UserId};

/// Returns a vector of all guild members with the specified role ID.
#[expect(dead_code)]
async fn guild_members_with_role(
    ctx: &AppContext<'_>,
    guild: &PartialGuild,
    role_id: RoleId,
) -> Result<Vec<UserId>> {
    // max number of pages to try to fetch (to avoid infinite loops in the event of the server
    // response format changing in a way that breaks the end-of-page detection)
    const MAX_PAGES: u64 = 20;

    // maximum number of members to request per page
    const PAGE_LIMIT: u64 = 1000;

    let mut last_member = None;
    let mut members_with_role = Vec::new();

    for _ in 0..MAX_PAGES {
        let page = guild.members(&ctx, Some(PAGE_LIMIT), last_member).await?;

        members_with_role.extend(
            page.iter()
                .filter(move |u| u.roles.iter().contains(&role_id))
                .map(|p| p.user.id),
        );

        if page.len() < PAGE_LIMIT as usize {
            break;
        }
        last_member = Some(
            page.last()
                .expect("page is never empty here if PAGE_LIMIT > 0")
                .user
                .id,
        );
    }

    Ok(members_with_role)
}

/// Pairs members with ROLE_NAME in the guild together.
/// The result is a pairing of
pub async fn match_members(ctx: AppContext<'_>, seed: u64) -> Result<Pairing<UserId>> {
    let participants = get_current_opted_in(ctx.data()).await?;
    if participants.len() <= 1 {
        bail!(
            "Need at least two members to create a pairing (found {}).",
            participants.len()
        );
    }
    graph_pair(participants, &get_previous_matches(ctx.data()).await?, seed)
}
