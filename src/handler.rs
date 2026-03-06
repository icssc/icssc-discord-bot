use crate::AppVars;
use crate::attendance::checkin::confirm_attendance_log_modal;
use crate::bitsnbytes::meetup::confirm_bnb_meetup_modal;
use crate::matchy::opt_in::MatchyMeetupOptIn;
use crate::spottings::check_victim::check_message_snipe_victim;
use crate::spottings::log::confirm_message_spotting_modal;
use crate::spottings::privacy::SnipesOptOut;
use crate::spottings::socials_role::SocialsParticipation;
use crate::util::text::bot_invite_url;
use rand::seq::IndexedRandom as _;
use serenity::all::{
    ActivityData, ActivityType, CacheHttp as _, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditInteractionResponse, EventHandler, Interaction, Message,
    OnlineStatus, Permissions, Ready,
};
use serenity::async_trait;
use std::time::Duration;
use tokio::time;

pub(crate) struct LaikaEventHandler {
    pub(crate) data: AppVars,
}

#[async_trait]
impl EventHandler for LaikaEventHandler {
    async fn message(&self, ctx: serenity::all::Context, new_message: Message) {
        // call all appropriate handlers for a message
        // parallelize if needed in the future
        let _ = check_message_snipe_victim(&ctx, &self.data, &new_message).await;
    }

    async fn ready(&self, ctx: serenity::all::Context, data_about_bot: Ready) {
        println!(
            "ok, connected as {} (UID {})",
            data_about_bot.user.tag(),
            data_about_bot.user.id
        );
        println!("using discord API version {}", data_about_bot.version);
        println!(
            "invite link: {}",
            bot_invite_url(data_about_bot.user.id, Permissions::empty(), true)
        );

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(120));

            let status = [
                "spotting ICSSC members",
                "getting countersniped",
                "sneaking around",
                "taking out my phone",
                "sign up for matchy meetups!",
                "setting up matchy meetups",
                "visit icssc.club!",
                "come to ICSSC events!",
                "you can just build things",
                "you can just do things",
                "you can just spot people",
            ];

            loop {
                ctx.shard.set_presence(
                    Some(ActivityData {
                        name: String::from("bazinga"),
                        kind: ActivityType::Custom,
                        state: Some(String::from(*status.choose(&mut rand::rng()).unwrap())),
                        url: None,
                    }),
                    OnlineStatus::Idle,
                );
                interval.tick().await;
            }
        });
        println!("status cycling active");
    }

    async fn interaction_create(&self, ctx: serenity::all::Context, interaction: Interaction) {
        let response = match &interaction {
            Interaction::Component(interaction) => match interaction.data.custom_id.as_str() {
                "matchy_opt_in" => {
                    MatchyMeetupOptIn::new(&ctx, &self.data)
                        .join(interaction)
                        .await
                }
                "matchy_opt_out" => {
                    MatchyMeetupOptIn::new(&ctx, &self.data)
                        .leave(interaction)
                        .await
                }
                "matchy_check_participation" => {
                    MatchyMeetupOptIn::new(&ctx, &self.data)
                        .check(interaction)
                        .await
                }
                "snipes_opt_in" => {
                    SnipesOptOut::new(&ctx, &self.data)
                        .opt_in(interaction)
                        .await
                }
                "snipes_opt_out" => {
                    SnipesOptOut::new(&ctx, &self.data)
                        .opt_out(interaction)
                        .await
                }
                "snipes_check_participation" => {
                    SnipesOptOut::new(&ctx, &self.data).check(interaction).await
                }
                "socials_opt_in" => {
                    SocialsParticipation::new(&ctx, &self.data)
                        .opt_in(interaction)
                        .await
                }
                "socials_opt_out" => {
                    SocialsParticipation::new(&ctx, &self.data)
                        .opt_out(interaction)
                        .await
                }
                "socials_check_participation" => {
                    SocialsParticipation::new(&ctx, &self.data)
                        .check(interaction)
                        .await
                }
                _ => Ok(()),
            },
            Interaction::Modal(interaction) => match interaction.data.custom_id.as_str() {
                "spotting_modal_confirm" => {
                    confirm_message_spotting_modal(&ctx, &self.data, interaction).await
                }
                "attendance_log_modal_confirm" => {
                    confirm_attendance_log_modal(&ctx, &self.data, interaction).await
                }
                "bnb_meetup_log_modal" => {
                    confirm_bnb_meetup_modal(&ctx, &self.data, interaction).await
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        };

        let Err(error) = response else {
            return;
        };

        dbg!(&error);
        let http = ctx.http();

        let new_response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(error.to_string())
                .ephemeral(true),
        );
        let did_create = match &interaction {
            Interaction::Command(ixn) | Interaction::Autocomplete(ixn) => {
                ixn.create_response(ctx.http(), new_response).await
            }
            Interaction::Component(ixn) => ixn.create_response(ctx.http(), new_response).await,
            Interaction::Modal(ixn) => ixn.create_response(ctx.http(), new_response).await,
            _ => return,
        }
        .is_ok();

        let edit_response = match did_create {
            true => return,
            false => EditInteractionResponse::new().content(error.to_string()),
        };

        let _ = match interaction {
            Interaction::Command(ixn) | Interaction::Autocomplete(ixn) => {
                ixn.edit_response(http, edit_response).await
            }
            Interaction::Component(ixn) => ixn.edit_response(http, edit_response).await,
            Interaction::Modal(ixn) => ixn.edit_response(http, edit_response).await,
            _ => return,
        };
    }
}
