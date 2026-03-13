use std::{
    ops::Add as _,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppVars, Vars, VarsServiceAccountKey};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct TokenResponse {
    access_token: String,
    // Can also read token_type: String
    expires_in: i64,
}

pub(crate) struct GoogleServiceAccount {
    account_key: VarsServiceAccountKey,
    last_response: Option<TokenResponse>,
    last_scope: String,
    expires: DateTime<Utc>,
}

impl GoogleServiceAccount {
    pub(crate) fn new(env: &Vars) -> GoogleServiceAccount {
        GoogleServiceAccount {
            account_key: env.service_account_key.clone(),
            last_response: None,
            expires: Utc::now(),
            last_scope: String::from(""),
        }
    }

    pub(crate) async fn get_access_token(&mut self, scope: &str) -> Result<String, AppError> {
        let key = &self.account_key;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let now = now.as_secs();

        if self.expires > Utc::now()
            && scope == self.last_scope
            && let Some(resp) = &self.last_response
        {
            return Ok(resp.access_token.clone());
        }

        let claims = Claims {
            iss: key.email.clone(),
            sub: key.email.clone(),
            scope: scope.to_owned(),
            aud: "https://oauth2.googleapis.com/token".to_owned(),
            exp: now + 3600,
            iat: now,
        };

        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(key.id.clone());

        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(key.pem.as_bytes())?,
        )?;

        let token_resp = reqwest::Client::new()
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &token),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        let access_token = token_resp.access_token.clone();
        self.expires = Utc::now().add(Duration::seconds(token_resp.expires_in));
        self.last_response = Some(token_resp);
        scope.clone_into(&mut self.last_scope);

        Ok(access_token)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DriveFilePermissionRole {
    Owner,
    Organizer,     // Manager
    FileOrganizer, // Content Manager
    Writer,        // Editor
    Commenter,
    Reader,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriveFilePermission {
    pub(crate) email_address: String,
    pub(crate) role: DriveFilePermissionRole,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PermissionsResponse {
    pub(crate) permissions: Vec<DriveFilePermission>,
    next_page_token: Option<String>,
}

async fn get_permissions_page(
    data: &AppVars,
    access_token: &str,
    page_token: Option<&str>,
) -> Result<PermissionsResponse, AppError> {
    let mut query = vec![
        ("fields", "nextPageToken,permissions(role,emailAddress)"),
        ("supportsTeamDrives", "true"),
    ];

    if let Some(tok) = page_token {
        query.push(("pageToken", tok));
    }

    let resp = data
        .http
        .client
        .get(format!(
            "https://www.googleapis.com/drive/v3/files/{}/permissions",
            data.env.roster_spreadsheet.id
        ))
        .query(&query)
        .bearer_auth(access_token)
        .send()
        .await?
        .json::<PermissionsResponse>()
        .await?;

    Ok(resp)
}

pub(crate) async fn get_gdrive_permissions(
    data: &AppVars,
) -> Result<Vec<DriveFilePermission>, AppError> {
    let access_token = data
        .google_service_account
        .write()
        .await
        .get_access_token("https://www.googleapis.com/auth/drive.metadata.readonly")
        .await?;

    let mut next_page_token = None;
    let mut permissions = vec![];

    loop {
        let resp = get_permissions_page(data, &access_token, next_page_token.as_deref()).await?;

        permissions.extend(resp.permissions);
        match resp.next_page_token {
            Some(tok) => next_page_token = Some(tok),
            None => break,
        }
    }

    Ok(permissions)
}
