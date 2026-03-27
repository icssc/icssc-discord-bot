use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use actix_web::{App, HttpServer, ResponseError, web};
use anyhow::Context as _;
use serenity::all::Http;

use crate::AppVarsInner;
use crate::routes;

#[derive(Clone)]
pub(crate) struct ActixData {
    pub(crate) discord_http: Arc<Http>,
    pub(crate) vars: Arc<AppVarsInner>,
}
pub(crate) type ExtractedAppData = web::Data<ActixData>;

#[repr(transparent)]
#[derive(Debug)]
pub(crate) struct AnyhowBridge(anyhow::Error);

impl Display for AnyhowBridge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for AnyhowBridge
where
    T: Into<anyhow::Error>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

pub(crate) type Result<T> = std::result::Result<T, AnyhowBridge>;

impl ResponseError for AnyhowBridge {}

pub(crate) async fn run(vars: Arc<AppVarsInner>, http_action: Arc<Http>) -> anyhow::Result<()> {
    let port = vars.http.port;
    let app_data = ActixData {
        discord_http: http_action,
        vars,
    };

    let server = {
        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(app_data.clone()))
                .service(web::scope("/check").service(routes::status_check::check_discord_conn))
        })
        .bind(("::", port))
        .with_context(|| format!("failed to bind to port {port}"))
    }
    .expect("Start server");

    println!("Listening on port {port}...");

    Ok(server.run().await?)
}
