use crate::util::ContextExtras as _;
use crate::{AppError, AppVars, AppVarsInner, Vars, meta, roster, shortlink};
use crate::{attendance, bitsnbytes, matchy, spottings};
use clap::ArgMatches;
use itertools::Itertools as _;
use pluralizer::pluralize;
use poise::{BoxFuture, Command, Framework, FrameworkError, FrameworkOptions};
use serenity::FutureExt as _;
use serenity::all::GuildId;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn load_env(args: &ArgMatches) {
    let _ = dotenvy::from_filename(
        args.get_one::<PathBuf>("config")
            .expect("config file is bad path?"),
    );
}

// Env Setup
pub(crate) struct ChannelVars {
    pub(crate) icssc_guild_id: u64,
    pub(crate) matchy_channel_id: u64,
    pub(crate) spottings_channel_id: u64,
}

impl ChannelVars {
    pub(crate) fn new(env: &Vars) -> Self {
        Self {
            icssc_guild_id: env
                .bot
                .channels
                .icssc_guild_id
                .parse::<_>()
                .expect("BOT__CHANNELS__ICSSC_GUILD_ID must be valid u64"),
            matchy_channel_id: env
                .bot
                .channels
                .matchy
                .parse::<_>()
                .expect("BOT__CHANNELS__MATCHY must be valid u64"),
            spottings_channel_id: env
                .bot
                .channels
                .spottings
                .parse::<_>()
                .expect("BOT__CHANNELS__SPOTTINGS must be valid u64"),
        }
    }
}

pub(crate) struct RoleVars {
    pub(crate) socials_role_id: u64,
}
impl RoleVars {
    pub(crate) fn new(env: &Vars) -> Self {
        Self {
            socials_role_id: env
                .bot
                .roles
                .socials_ping
                .parse::<_>()
                .expect("BOT__ROLES__SOCIALS__PING must be valid u64"),
        }
    }
}

pub(crate) struct HttpVars {
    pub(crate) port: u16,
    pub(crate) client: reqwest::Client,
}

impl HttpVars {
    pub(crate) fn new(env: &Vars) -> Self {
        let port = env
            .app
            .port
            .parse::<u16>()
            .expect("$PORT not valid u16 port");

        Self {
            port,
            client: reqwest::Client::new(),
        }
    }
}

// Bot setup

pub(crate) async fn register_commands(
    data: Arc<AppVarsInner>,
    ctx: &serenity::all::Context,
    framework: &Framework<AppVars, AppError>,
) -> Result<(), AppError> {
    let is_global = !data.env.bot.commands.register_globally.is_empty();
    let no_commands = &[] as &[Command<AppVars, AppError>];
    let commands = &framework.options().commands;
    let global_registration = if is_global { commands } else { no_commands };
    let local_registration = if is_global { no_commands } else { commands };
    let guilds = data
        .env
        .bot
        .commands
        .guilds
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|id| GuildId::from(id.parse::<u64>().expect("guild id not valid snowflake")))
        .collect_vec();

    poise::builtins::register_globally(ctx, global_registration).await?;

    for id in &guilds {
        poise::builtins::register_in_guild(ctx, local_registration, *id).await?;
    }

    let commands_text = pluralize("command", framework.options().commands.len() as isize, true);
    if is_global {
        println!("[setup] Registered {commands_text} globally");
    } else {
        let guilds_text = pluralize("guild", guilds.len() as isize, true);
        println!("[setup] Registered {commands_text} locally in {guilds_text}");
    }

    Ok(())
}

fn handle_framework_error(error: FrameworkError<'_, AppVars, AppError>) -> BoxFuture<'_, ()> {
    async move {
        println!("Error: {error}");

        let Some(ctx) = error.ctx() else { return };
        let error_res = match error {
            FrameworkError::Command {
                error: wrapped_error,
                ..
            } => {
                ctx.reply_ephemeral(format!("Error: {wrapped_error:?}"))
                    .await
            }
            _ => ctx.reply_ephemeral("An unknown error occurred").await,
        };
        if let Err(e) = error_res {
            println!("A further error occurred sending the error message to discord: {e:?}");
        }
    }
    .boxed()
}

// fn check_command_invocation(
//     ctx: poise::Context<AppVars, AppError>,
// ) -> BoxFuture<Result<bool, AppError>> {
//     const ICSSC_SERVER: u64 = 760915616793755669;
//     const ALLOWED_CHANNELS: &[u64] = &[1328907402321592391, 1338632123929591970];
//
//     async move {
//         Ok(ctx.guild_id() != Some(GuildId::from(ICSSC_SERVER))
//             || ALLOWED_CHANNELS.contains(&ctx.channel_id().into()))
//     }
//     .boxed()
// }

fn get_bot_commands() -> Vec<Command<AppVars, AppError>> {
    vec![
        attendance::attended::attended(),
        attendance::checkin::checkin(),
        attendance::checkin::log_attendance(),
        bitsnbytes::meetup::log_bnb_meetup_message(),
        matchy::command::matchy(),
        meta::ping::ping(),
        roster::user_lookup::user_lookup(),
        roster::command::roster(),
        spottings::command::spottings(),
        spottings::remove_spotting::remove_spotting(),
        spottings::log::log_message_spotting(),
        shortlink::command::shortlink(),
    ]
}

pub(crate) fn create_bot_framework_options() -> FrameworkOptions<AppVars, AppError> {
    FrameworkOptions {
        on_error: handle_framework_error,
        commands: get_bot_commands(),
        // command_check: Some(check_command_invocation),
        ..Default::default()
    }
}
