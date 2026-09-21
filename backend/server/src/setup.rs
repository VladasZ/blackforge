use blackforge_api::setup::{SaveResult, SaveSetup, SavedSetup, SetupAccount};
use hilen_server::{
    AppError,
    auth::User,
    axum::{Json, Router, extract::State, routing::get},
};
use serde_json::{from_str, to_string};
use sqlx::PgPool;

pub fn routes() -> Router<PgPool> {
    Router::new().route("/api/setup", get(read).put(save))
}

async fn read(user: User, State(db): State<PgPool>) -> Result<Json<SetupAccount>, AppError> {
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT revision, setup FROM cloud_setups WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&db)
            .await?;
    let saved = row
        .map(|(revision, json)| from_str(&json).map(|setup| SavedSetup { revision, setup }))
        .transpose()
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(Json(SetupAccount {
        account: user.id.to_string(),
        saved,
    }))
}

async fn save(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<SaveSetup>,
) -> Result<Json<SaveResult>, AppError> {
    if body.revision < 0 || body.revision == i64::MAX {
        return Err(AppError::BadRequest("invalid setup revision".to_owned()));
    }
    let json = to_string(&body.setup).map_err(|error| AppError::Internal(error.into()))?;
    let row: Option<(i64,)> = if body.revision == 0 {
        sqlx::query_as(
            r"INSERT INTO cloud_setups (user_id, revision, setup)
VALUES ($1, 1, $2) ON CONFLICT DO NOTHING RETURNING revision",
        )
        .bind(user.id)
        .bind(json)
        .fetch_optional(&db)
        .await?
    } else {
        sqlx::query_as(
            r"UPDATE cloud_setups SET revision = revision + 1, setup = $2, updated_at = now()
WHERE user_id = $1 AND revision = $3 RETURNING revision",
        )
        .bind(user.id)
        .bind(json)
        .bind(body.revision)
        .fetch_optional(&db)
        .await?
    };
    Ok(Json(match row {
        Some((revision,)) => SaveResult::Saved(SavedSetup {
            revision,
            setup: body.setup,
        }),
        None => SaveResult::Conflict,
    }))
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use hilen_server::axum::{http::StatusCode, serve};
    use reqwest::Client;
    use sqlx::postgres::PgPoolOptions;
    use tokio::{net::TcpListener, spawn, sync::oneshot};

    use super::routes;

    #[tokio::test]
    async fn private_setup_routes_refuse_anonymous_reads_and_writes() -> Result<()> {
        let db = PgPoolOptions::new().connect_lazy("postgres://localhost/unused")?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, stopped) = oneshot::channel();
        let server = spawn(async move {
            serve(listener, routes().with_state(db))
                .with_graceful_shutdown(async {
                    stopped.await.expect("test sends shutdown");
                })
                .await
        });
        let client = Client::new();
        let url = format!("http://{address}/api/setup");
        assert_eq!(
            client.get(&url).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client.put(&url).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
        stop.send(()).expect("server is alive");
        server.await??;
        Ok(())
    }
}
