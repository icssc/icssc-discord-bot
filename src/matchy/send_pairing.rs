use super::discord_helpers::match_members;
use super::helpers::{Pairing, add_pairings_to_db, checksum_matching, format_pairs, hash_seed};
use crate::AppContext;
use crate::util::text::remove_markdown;
use anyhow::{Context as _, Error, Result, bail, ensure};
use itertools::Itertools as _;
use poise::futures_util::future::try_join_all;
use serenity::all::{GuildId, Mentionable as _, UserId};
use std::collections::HashSet;

/// Run the /send_pairing command
async fn handle_send_pairing(ctx: AppContext<'_>, key: String) -> Result<String> {
    let Some((seed_str, checksum)) = key.rsplit_once('_') else {
        bail!("Invalid key. Please make sure you only use keys returned by `/matchy create`")
    };

    let channels = &ctx.data().channels;

    let channel_map = GuildId::from(channels.icssc_guild_id)
        .channels(ctx.http())
        .await
        .context("get channel map in ICSSC_GUILD_ID")?;
    let Some(notification_channel) = channel_map.get(&channels.matchy_channel_id.into()) else {
        bail!("Could not find notification channel");
    };

    let seed = hash_seed(seed_str);

    let Pairing(pairs, _) = match_members(ctx, seed).await?;
    let pairs_str = format_pairs(&pairs);
    ensure!(
        checksum_matching(seed, &pairs) == checksum,
        "Key mismatch. This can happen if you typed the key incorrectly, or the members with the \
        matchy meetups role have changed since this key was generated. Please call `/matchy create` \
        again to get a new key."
    );

    add_pairings_to_db(&ctx, pairs.clone()).await?;

    notification_channel
        .say(
            &ctx,
            format!(
                "Hey all, here are the pairings for the next round of matchy meetups!\n\n{pairs_str}"
            ),
        )
        .await?;

    let mut messages_sent = 0;

    let mut failed_to_send = HashSet::new();

    for pair in pairs {
        for user in &pair {
            let pairing: Vec<_> = pair.iter().filter(|u| *u != user).collect();
            let pairing_str = try_join_all(pairing.iter().map(|uid| async {
                let u = uid.to_user(&ctx).await?;
                Ok::<String, Error>(format!(
                    "<@{}> ({})",
                    u.id,
                    remove_markdown(&u.global_name.unwrap_or(u.name))
                ))
            }))
            .await
            .context("Unable to fetch names for user ids")?
            .join(" and ");

            let message_str = format!(
                "Hey, thanks for joining ICSSC's Matchy Meetups. Your pairing \
                 for this round is here! Please take this opportunity to reach out to them and \
                 schedule some time to hang out in the next two weeks. \
                 Don't forget to send pics to https://discord.com/channels/760915616793755669/1199228930222194779 \
                 while you're there, and I hope you enjoy!\n\
                 \t\t\t\t\t\t\t \\- Ethan \n\n\n\
                 **Your pairing is with:** {pairing_str}\n\n\
                 _(responses here will not be seen; please message Ethan (@awesome\\_e) directly if you have any questions)_"
            );
            let success = {
                match user.create_dm_channel(&ctx).await {
                    Ok(ch) => ch.say(&ctx, message_str).await.ok(),
                    Err(_) => None,
                }
            };

            if success.is_none() {
                failed_to_send.insert(*user);
            } else {
                messages_sent += 1;
            }
        }
    }

    Ok(match failed_to_send.len() {
        0 => format!("Successfully messaged {messages_sent} users."),
        1.. => format!(
            "Successfully messaged {} users, but failed for the following users: {}",
            messages_sent,
            failed_to_send.iter().map(UserId::mention).join(", ")
        ),
    })
}

/// Send a message to each member of the pairing.
#[poise::command(
    slash_command,
    hide_in_help,
    rename = "send",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn send_pairing(
    ctx: AppContext<'_>,
    #[description = "A pairing key returned by /create_pairing."] key: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let resp = handle_send_pairing(ctx, key)
        .await
        .unwrap_or_else(|e| format!("Error: {e}"));
    ctx.say(resp).await?;
    Ok(())
}
