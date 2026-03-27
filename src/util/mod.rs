pub(crate) mod gdrive;
pub(crate) mod gforms;
pub(crate) mod gsheets;
pub(crate) mod message;
pub(crate) mod modal;
pub(crate) mod paginate;
pub(crate) mod roster;
pub(crate) mod text;

use crate::AppContext;
use poise::{CreateReply, ReplyHandle};
use serenity::all::{CreateEmbed, CreateEmbedAuthor, User};

pub(crate) fn base_embed(ctx: &serenity::all::Context) -> CreateEmbed {
    CreateEmbed::default()
        .color(0xff87a6)
        .author(CreateEmbedAuthor::from(User::from(
            ctx.cache.current_user().clone(),
        )))
}

pub(crate) fn spottings_embed() -> CreateEmbed {
    CreateEmbed::default()
        .color(0xff87a6)
        .author(CreateEmbedAuthor::new("ICS Spottings Council")
            .icon_url("https://cdn.discordapp.com/avatars/1336510972403126292/8db135d66c041c0191e0ae8085b9baa6.webp?size=512"))
}

pub trait ContextExtras<'a> {
    async fn reply_ephemeral(
        self,
        text: impl Into<String>,
    ) -> Result<ReplyHandle<'a>, serenity::Error>;
}

impl<'a> ContextExtras<'a> for AppContext<'a> {
    async fn reply_ephemeral(
        self,
        text: impl Into<String>,
    ) -> Result<ReplyHandle<'a>, serenity::Error> {
        self.send(
            CreateReply::default()
                .content(text)
                .reply(true)
                .ephemeral(true),
        )
        .await
    }
}
