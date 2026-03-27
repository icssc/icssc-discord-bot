mod attendance;
mod bitsnbytes;
mod handler;
mod matchy;
mod meta;
mod roster;
mod routes;
mod server;
mod setup;
mod shortlink;
mod spottings;
mod util;

use crate::setup::{
    ChannelVars, HttpVars, RoleVars, create_bot_framework_options, register_commands,
};
use crate::util::gdrive::GoogleServiceAccount;
use crate::util::roster::Roster;
use anyhow::Context as _;
use clap::ValueHint;
use env_vars_struct::env_vars_struct;
use migration::{Migrator, MigratorTrait as _};
use serenity::Client;
use serenity::all::GatewayIntents;
use std::env;
use std::ops::{BitOr as _, Deref};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

env_vars_struct!(
    "APP__DATABASE_URL",
    "APP__JWT_SECRET",
    "APP__ORIGIN",
    "APP__PORT",
    "ATTENDANCE_FORM__ID",
    "ATTENDANCE_FORM__TOKEN_INPUT_ID",
    "ATTENDANCE_FORM__TOKEN_INPUT_VALUE",
    "ATTENDANCE_FORM__EVENT_INPUT_ID",
    "ATTENDANCE_SHEET__ID",
    "ATTENDANCE_SHEET__RANGES__CHECKIN",
    "BNB_FORM__ID",
    "BNB_FORM__INPUT_IDS__FAM_NAME",
    "BNB_FORM__INPUT_IDS__MSG_LINK",
    "BNB_FORM__INPUT_IDS__MEETUP_TYPE",
    "BNB_SHEET__ID",
    "BNB_SHEET__LOOKUP_RANGE",
    "BOT__COMMANDS__REGISTER_GLOBALLY",
    "BOT__COMMANDS__GUILDS",
    "BOT__CHANNELS__ICSSC_GUILD_ID",
    "BOT__CHANNELS__MATCHY",
    "BOT__CHANNELS__SPOTTINGS",
    "BOT__DISCORD_TOKEN",
    "BOT__ROLES__SOCIALS_PING",
    "GOOGLE_OAUTH_CLIENT__ID",
    "GOOGLE_OAUTH_CLIENT__SECRET",
    "ROSTER_SPREADSHEET__ID",
    "ROSTER_SPREADSHEET__RANGE",
    "SERVICE_ACCOUNT_KEY__ID",
    "SERVICE_ACCOUNT_KEY__EMAIL",
    "SERVICE_ACCOUNT_KEY__PEM",
    "SHORTLINK__SECRET",
    "SHORTLINK__STYLE_GUIDE_URL",
);

struct AppVarsInner {
    env: Vars,
    db: sea_orm::DatabaseConnection,
    channels: ChannelVars,
    roles: RoleVars,
    google_service_account: Arc<RwLock<GoogleServiceAccount>>,
    roster: RwLock<Roster>,
    http: HttpVars,
}

#[derive(Clone)]
struct AppVars {
    inner: std::sync::Arc<AppVarsInner>,
}

impl Deref for AppVars {
    type Target = AppVarsInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AppVars {
    async fn new() -> Self {
        let env = Vars::new();

        let connection = {
            let db_url = &env.app.database_url;
            sea_orm::Database::connect(db_url).await.unwrap()
        };

        let google_service_account = Arc::new(RwLock::new(GoogleServiceAccount::new(&env)));

        Self {
            inner: std::sync::Arc::new(AppVarsInner {
                db: connection,
                channels: ChannelVars::new(&env),
                http: HttpVars::new(&env),
                roles: RoleVars::new(&env),
                roster: RwLock::new(Roster::new(
                    &env.roster_spreadsheet,
                    google_service_account.clone(),
                )),
                google_service_account,
                env,
            }),
        }
    }
}

#[tokio::main]
async fn main() {
    let cmd = clap::command!("icssc-discord-bot")
        .about("The somewhat official Discord bot for ICS Student Council")
        .arg(clap::arg!(--migrate "migrate db"))
        .arg(
            clap::arg!(--config <PATH> ".env file path")
                .value_parser(clap::value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath)
                .default_value(".env"),
        );

    let args = cmd.get_matches();
    setup::load_env(&args);

    let data = AppVars::new().await;
    let inner_vars = data.inner.clone();

    if args.get_flag("migrate") {
        Migrator::up(&data.db, None)
            .await
            .expect("Migration failed");
        return;
    }

    let framework = poise::Framework::<AppVars, AppError>::builder()
        .options(create_bot_framework_options())
        .setup({
            let data = data.clone();
            |ctx, _ready, framework| {
                Box::pin(async move {
                    register_commands(data.inner.clone(), ctx, framework).await?;
                    Ok(data)
                })
            }
        })
        .build();

    let mut client = Client::builder(
        &data.env.bot.discord_token,
        GatewayIntents::non_privileged()
            .bitor(GatewayIntents::GUILD_MEMBERS)
            .bitor(GatewayIntents::MESSAGE_CONTENT),
    )
    .event_handler(handler::LaikaEventHandler { data })
    .framework(framework)
    .await
    .expect("couldn't make client");

    let http_action = client.http.clone();

    let serenity_task = async move {
        client.start().await.context("start serenity")?;
        anyhow::Result::<()>::Ok(())
    };

    let actix_task = async move {
        crate::server::run(inner_vars, http_action)
            .await
            .context("start actix")?;
        anyhow::Result::<()>::Ok(())
    };

    tokio::select! {
        biased;

        _ = tokio::signal::ctrl_c() => {
            println!("SIGINT, going down");
        }

        _ = serenity_task => {
            println!("serenity has stopped");
        }

        _ = actix_task => {
            println!("actix has stopped");
        }
    }
}

type AppError = anyhow::Error;
type AppContext<'a> = poise::Context<'a, AppVars, AppError>;
