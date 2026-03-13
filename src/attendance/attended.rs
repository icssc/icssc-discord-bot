use anyhow::Error;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use itertools::Itertools as _;

use crate::{
    AppContext, AppError, AppVars,
    util::{ContextExtras as _, gsheets::get_spreadsheet_range, roster::get_user_from_discord},
};

pub(crate) async fn get_events_attended_text(
    data: &AppVars,
    email: &String,
) -> Result<Vec<String>, AppError> {
    let sheet_id = &data.env.attendance_sheet.id;
    let range = &data.env.attendance_sheet.ranges.checkin;
    let resp = get_spreadsheet_range(data, sheet_id, range).await?;

    let events = resp
        .values
        .into_iter()
        .filter_map(|row| {
            let row = row.into_iter().collect_array::<4>()?;
            let [time, row_email, _, name] = row;

            if row_email != *email {
                return None;
            }

            let current_time = Utc::now().time();
            let datetime = NaiveDateTime::parse_from_str(&time, "%m/%d/%Y %H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(&time, "%m/%d/%y %H:%M:%S"))
                .or_else(|_| {
                    NaiveDate::parse_from_str(&time, "%m/%d/%y")
                        .map(|res| res.and_time(current_time))
                })
                .or_else(|_| {
                    NaiveDate::parse_from_str(&time, "%m/%d/%Y")
                        .map(|res| res.and_time(current_time))
                })
                .ok()?;

            Some(format!("- <t:{}:d> {name}", datetime.and_utc().timestamp()))
        })
        .collect_vec();

    Ok(events)
}

/// See what ICSSC events you have checked in for!
#[poise::command(slash_command, hide_in_help)]
pub(crate) async fn attended(ctx: AppContext<'_>) -> Result<(), Error> {
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

    let events = get_events_attended_text(ctx.data(), &user.email).await?;

    ctx.reply_ephemeral(format!("Events you attended:\n{}", events.join("\n")))
        .await?;
    Ok(())
}
