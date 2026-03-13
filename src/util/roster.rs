use std::collections::HashSet;

use anyhow::anyhow;
use itertools::Itertools as _;
use serde::Deserialize;

use crate::{
    AppError, AppVars,
    util::{
        gforms::submit_google_form,
        gsheets::{SheetsResponse, get_spreadsheet_range},
    },
};

#[derive(Debug, Deserialize)]
pub(crate) struct RosterSheetRow {
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) discord: String,
    pub(crate) committees: Vec<String>,
}

impl RosterSheetRow {
    pub fn is_board(&self) -> bool {
        self.committees.iter().any(|c| c == "board")
    }
}

fn parse_committees_string(committees_text: &str) -> Vec<String> {
    committees_text
        .split(", ")
        .map(|val| val.to_lowercase().replace('_', ""))
        .collect_vec()
}

async fn get_roster_rows(data: &AppVars) -> Result<SheetsResponse, AppError> {
    let spreadsheet = &data.env.roster_spreadsheet;

    let resp = get_spreadsheet_range(data, &spreadsheet.id, &spreadsheet.range).await?;

    Ok(resp)
}

pub(crate) async fn get_user_from_discord(
    data: &AppVars,
    username: String,
) -> Result<Option<RosterSheetRow>, AppError> {
    let resp = get_roster_rows(data).await?;

    let user = resp
        .values
        .into_iter()
        .filter_map(|row| {
            let [name, email, discord, committees] = row.into_iter().collect_array::<4>()?;
            let committees = parse_committees_string(&committees);
            Some(RosterSheetRow {
                name,
                email,
                discord,
                committees,
            })
        })
        .find(|row| row.discord.to_lowercase() == username);

    Ok(user)
}

pub(crate) async fn get_bulk_members_from_roster(
    data: &AppVars,
    usernames: &[String],
) -> Result<Vec<RosterSheetRow>, AppError> {
    let usernames: HashSet<&String> = usernames.iter().collect();
    let resp = get_roster_rows(data).await?;

    let rows = resp
        .values
        .into_iter()
        .filter_map(|row| {
            let [name, email, discord, committees] = row.into_iter().collect_array::<4>()?;
            let committees = parse_committees_string(&committees);

            let found = usernames.is_empty() || usernames.contains(&discord.to_lowercase());
            if !found {
                return None;
            }

            Some(RosterSheetRow {
                name,
                email,
                discord,
                committees,
            })
        })
        .collect_vec();

    Ok(rows)
}

pub(crate) async fn check_in_with_email(
    data: &AppVars,
    email: &str,
    reason: Option<&str>,
) -> Result<(), AppError> {
    let fields = &data.env.attendance_form;

    let mut payload = vec![
        ("emailAddress", email),
        (&fields.token_input_id, &fields.token_input_value),
    ];

    if let Some(reason) = reason {
        payload.push((&fields.event_input_id, reason));
    }

    submit_google_form(&data.http.client, &fields.id, &payload)
        .await
        .map_err(|err| {
            dbg!(err);
            anyhow!("Google Form submission failed. Please check your inputs.")
        })?;

    Ok(())
}
