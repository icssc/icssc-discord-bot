use serde::Deserialize;

use crate::AppVars;

#[derive(Debug, Deserialize)]
pub(crate) struct SheetsResponse {
    pub(crate) values: Vec<Vec<String>>,
}

pub(crate) async fn get_spreadsheet_range(
    data: &AppVars,
    sheet_id: &str,
    range: &str,
) -> anyhow::Result<SheetsResponse> {
    let access_token = data
        .google_service_account
        .write()
        .await
        .get_access_token("https://www.googleapis.com/auth/spreadsheets.readonly")
        .await?;

    let resp = reqwest::Client::new()
        .get(format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{range}"
        ))
        .bearer_auth(access_token)
        .send()
        .await?
        .json::<SheetsResponse>()
        .await?;

    Ok(resp)
}
