//! Connection reports, what the game of a player saw when a join failed or a
//! connection dropped, see `docs/reports.md`. Any signed in player sends them,
//! only the admin reads them.

use blackforge_api::report::{ConnectionReport, LIST_LIMIT, ReportRow, StoredReport};
use hilen_server::tracing::info;
use hilen_server::{
    AppError,
    auth::User,
    axum::{
        Json, Router,
        extract::{Path, State},
        routing::get,
    },
};
use serde_json::{from_str, to_string};
use sqlx::PgPool;

use crate::routes::{require_admin, username_of};

/// A report older than this is deleted at the next report.
const KEEP_DAYS: i32 = 30;
/// One player keeps at most this many reports, a game stuck in a loop of
/// failed joins must not fill the disk.
const PER_USER: i64 = 500;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/api/reports", get(list).post(add))
        .route("/api/reports/{id}", get(one))
}

async fn add(
    user: User,
    State(db): State<PgPool>,
    Json(mut report): Json<ConnectionReport>,
) -> Result<Json<()>, AppError> {
    report.trim();
    let username = username_of(&db, user.id).await?.unwrap_or_default();
    info!(
        "connection report from {username} {}, character {}: {} {} after {:.1}s{}, {}",
        user.id,
        report.character,
        report.server,
        report.status,
        report.seconds,
        if report.in_world { " in the world" } else { "" },
        report.message
    );
    let json = to_string(&report).map_err(|error| AppError::BadRequest(error.to_string()))?;
    sqlx::query(
        r"INSERT INTO connection_reports
    (user_id, server, status, message, in_world, seconds, report, character)
VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8)",
    )
    .bind(user.id)
    .bind(&report.server)
    .bind(&report.status)
    .bind(&report.message)
    .bind(report.in_world)
    .bind(report.seconds)
    .bind(json)
    .bind(&report.character)
    .execute(&db)
    .await?;
    sqlx::query("DELETE FROM connection_reports WHERE created_at < now() - $1 * interval '1 day'")
        .bind(KEEP_DAYS)
        .execute(&db)
        .await?;
    sqlx::query(
        r"DELETE FROM connection_reports WHERE user_id = $1 AND id NOT IN (
    SELECT id FROM connection_reports WHERE user_id = $1 ORDER BY id DESC LIMIT $2)",
    )
    .bind(user.id)
    .bind(PER_USER)
    .execute(&db)
    .await?;
    Ok(Json(()))
}

/// The columns of one list row, in the order of the select.
type ListRow = (
    i64,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    String,
    bool,
    f64,
);

async fn list(user: User, State(db): State<PgPool>) -> Result<Json<Vec<ReportRow>>, AppError> {
    require_admin(&db, &user, "reads connection reports").await?;
    let rows: Vec<ListRow> = sqlx::query_as(
        r#"SELECT r.id, r.user_id::text, p.username,
    to_char(r.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    r.server, r.character, r.status, r.message, r.in_world, r.seconds
FROM connection_reports r LEFT JOIN profiles p ON p.user_id = r.user_id
ORDER BY r.id DESC LIMIT $1"#,
    )
    .bind(LIST_LIMIT)
    .fetch_all(&db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    user_id,
                    username,
                    created_at,
                    server,
                    character,
                    status,
                    message,
                    in_world,
                    seconds,
                )| ReportRow {
                    id,
                    user_id,
                    username: username.unwrap_or_default(),
                    created_at,
                    server,
                    character,
                    status,
                    message,
                    in_world,
                    seconds,
                },
            )
            .collect(),
    ))
}

async fn one(
    user: User,
    State(db): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<Json<StoredReport>, AppError> {
    require_admin(&db, &user, "reads connection reports").await?;
    let row: Option<(i64, String, Option<String>, String, String)> = sqlx::query_as(
        r#"SELECT r.id, r.user_id::text, p.username,
    to_char(r.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'), r.report::text
FROM connection_reports r LEFT JOIN profiles p ON p.user_id = r.user_id
WHERE r.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&db)
    .await?;
    let (id, user_id, username, created_at, report) = row.ok_or(AppError::NotFound)?;
    let report = from_str(&report).map_err(|error| AppError::BadRequest(error.to_string()))?;
    Ok(Json(StoredReport {
        id,
        user_id,
        username: username.unwrap_or_default(),
        created_at,
        report,
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
    use blackforge_api::report::ConnectionReport;

    #[tokio::test]
    /// Sending wants a login, reading too, and neither reaches the database
    /// here.
    async fn report_routes_refuse_anonymous_requests() -> Result<()> {
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
        let base = format!("http://{address}");
        let sent = client
            .post(format!("{base}/api/reports"))
            .json(&ConnectionReport::default())
            .send()
            .await?
            .status();
        assert_eq!(sent, StatusCode::UNAUTHORIZED);
        for path in ["/api/reports", "/api/reports/1"] {
            let read = client.get(format!("{base}{path}")).send().await?.status();
            assert_eq!(read, StatusCode::UNAUTHORIZED, "{path} without a login");
        }
        stop.send(()).expect("server is alive");
        server.await??;
        Ok(())
    }
}
