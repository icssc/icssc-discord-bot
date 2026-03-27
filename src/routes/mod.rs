pub(crate) mod status_check {
    use crate::server::ExtractedAppData;
    use actix_web::{HttpResponse, Responder, get};
    use serde_json::json;
    use serenity::all::GuildId;

    #[get("/")]
    async fn check_discord_conn(data: ExtractedAppData) -> crate::server::Result<impl Responder> {
        let guild_id = GuildId::from(data.vars.channels.icssc_guild_id);
        let guild_success = data.discord_http.get_guild(guild_id).await.is_ok();

        Ok(HttpResponse::Ok().json(json!({ "get_guild_success": guild_success })))
    }
}
