//! The blackforge server. It serves the landing page, the Google login of the
//! engine, the friends api of the app, and the list of broken mods.

mod broken;
mod routes;
mod servers;
mod site;
mod sync;

use std::env;

use anyhow::{Context, Result};
use hilen_server::{
    auth::{self, AuthConfig, AuthState, auth_routes},
    base_routes, build_db, serve, tracing_init,
};

const APP_NAME: &str = "Blackforge";
const DEFAULT_PORT: u16 = 3000;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init::init("blackforge");

    let port = env::var("PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL required")?;
    let broken = broken::load()?;

    let db = build_db(&database_url).await?;

    // `users` first, the tables of the app point at it.
    auth::migrate(&db).await?;
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("the blackforge migrations failed")?;

    let auth = AuthState::new(db.clone(), AuthConfig::from_env(APP_NAME)?);

    let app = base_routes("blackforge")
        .merge(auth_routes(auth))
        .merge(routes::routes())
        .merge(sync::routes())
        .merge(servers::routes())
        .merge(broken::routes(broken))
        .merge(site::routes())
        .with_state(db);

    serve(app, port).await
}
