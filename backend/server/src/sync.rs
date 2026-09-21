//! The private setup of an account and its full history. The head is the
//! newest applied revision, the one every machine installs.
use blackforge_api::setup::{Account, HistoryRow, Restore, Revision, Save, Saved, Setup, Summary};
use hilen_server::{
    AppError,
    auth::User,
    axum::{
        Json, Router,
        extract::State,
        routing::{get, post},
    },
};
use serde_json::{from_str, to_string};
use sqlx::{PgPool, Postgres, Transaction, types::Uuid};

const MACHINE_MAX: usize = 100;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/api/sync", get(head).post(save))
        .route("/api/sync/history", get(history))
        .route("/api/sync/restore", post(restore))
}

fn internal(error: serde_json::Error) -> AppError {
    AppError::Internal(error.into())
}

async fn head(user: User, State(db): State<PgPool>) -> Result<Json<Account>, AppError> {
    let row: Option<(i64, String, i64, String)> = sqlx::query_as(
        r"SELECT revision, machine, EXTRACT(EPOCH FROM created_at)::bigint, setup
FROM cloud_revisions WHERE user_id = $1 AND applied ORDER BY revision DESC LIMIT 1",
    )
    .bind(user.id)
    .fetch_optional(&db)
    .await?;
    let head = row
        .map(|(revision, machine, created, json)| {
            from_str(&json).map(|setup| Revision {
                revision,
                machine,
                created,
                setup,
            })
        })
        .transpose()
        .map_err(internal)?;
    Ok(Json(Account {
        account: user.id.to_string(),
        head,
    }))
}

struct Numbers {
    last: i64,
    head: i64,
}

/// Locks the account for the rest of the transaction, so the compare with the
/// head and the insert after it cannot interleave with another save.
async fn lock(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<Numbers, AppError> {
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    let (last, head): (i64, i64) = sqlx::query_as(
        r"SELECT COALESCE(MAX(revision), 0), COALESCE(MAX(revision) FILTER (WHERE applied), 0)
FROM cloud_revisions WHERE user_id = $1",
    )
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Numbers { last, head })
}

async fn setup_of(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    revision: i64,
) -> Result<Option<Setup>, AppError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT setup FROM cloud_revisions WHERE user_id = $1 AND revision = $2")
            .bind(user)
            .bind(revision)
            .fetch_optional(&mut **tx)
            .await?;
    row.map(|(json,)| from_str(&json))
        .transpose()
        .map_err(internal)
}

struct Insert<'a> {
    setup: &'a Setup,
    machine: &'a str,
    applied: bool,
    restored_from: Option<i64>,
}

async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    numbers: &Numbers,
    row: Insert<'_>,
) -> Result<i64, AppError> {
    let before = setup_of(tx, user, numbers.head).await?.unwrap_or_default();
    let summary = to_string(&Summary::between(&before, row.setup)).map_err(internal)?;
    let revision = numbers.last + 1;
    sqlx::query(
        r"INSERT INTO cloud_revisions (user_id, revision, setup, machine, applied, restored_from, summary)
VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user)
    .bind(revision)
    .bind(to_string(row.setup).map_err(internal)?)
    .bind(row.machine)
    .bind(row.applied)
    .bind(row.restored_from)
    .bind(summary)
    .execute(&mut **tx)
    .await?;
    Ok(revision)
}

fn check(base: i64, machine: &str) -> Result<(), AppError> {
    if base < 0 {
        return Err(AppError::BadRequest("invalid base revision".to_owned()));
    }
    if machine.chars().count() > MACHINE_MAX {
        return Err(AppError::BadRequest(
            "the machine name is too long".to_owned(),
        ));
    }
    Ok(())
}

async fn save(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<Save>,
) -> Result<Json<Saved>, AppError> {
    check(body.base, &body.machine)?;
    let mut tx = db.begin().await?;
    let numbers = lock(&mut tx, user.id).await?;
    // A setup that stays out of the head cannot overwrite anything, so it
    // needs no compare.
    if body.applied && body.base != numbers.head {
        return Ok(Json(Saved::Conflict));
    }
    let revision = insert(
        &mut tx,
        user.id,
        &numbers,
        Insert {
            setup: &body.setup,
            machine: &body.machine,
            applied: body.applied,
            restored_from: None,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(Saved::Saved { revision }))
}

async fn restore(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<Restore>,
) -> Result<Json<Saved>, AppError> {
    check(body.base, &body.machine)?;
    let mut tx = db.begin().await?;
    let numbers = lock(&mut tx, user.id).await?;
    if body.base != numbers.head {
        return Ok(Json(Saved::Conflict));
    }
    let setup = setup_of(&mut tx, user.id, body.revision)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("no revision {}", body.revision)))?;
    let revision = insert(
        &mut tx,
        user.id,
        &numbers,
        Insert {
            setup: &setup,
            machine: &body.machine,
            applied: true,
            restored_from: Some(body.revision),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(Saved::Saved { revision }))
}

async fn history(user: User, State(db): State<PgPool>) -> Result<Json<Vec<HistoryRow>>, AppError> {
    let rows: Vec<(i64, String, i64, bool, Option<i64>, String)> = sqlx::query_as(
        r"SELECT revision, machine, EXTRACT(EPOCH FROM created_at)::bigint, applied, restored_from, summary
FROM cloud_revisions WHERE user_id = $1 ORDER BY revision DESC",
    )
    .bind(user.id)
    .fetch_all(&db)
    .await?;
    rows.into_iter()
        .map(
            |(revision, machine, created, applied, restored_from, summary)| {
                Ok(HistoryRow {
                    revision,
                    machine,
                    created,
                    applied,
                    restored_from,
                    summary: from_str(&summary).map_err(internal)?,
                })
            },
        )
        .collect::<Result<_, _>>()
        .map(Json)
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
    async fn private_sync_routes_refuse_anonymous_reads_and_writes() -> Result<()> {
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
        let base = format!("http://{address}/api/sync");
        for url in [base.clone(), format!("{base}/history")] {
            assert_eq!(
                client.get(&url).send().await?.status(),
                StatusCode::UNAUTHORIZED
            );
        }
        for url in [base.clone(), format!("{base}/restore")] {
            assert_eq!(
                client.post(&url).send().await?.status(),
                StatusCode::UNAUTHORIZED
            );
        }
        stop.send(()).expect("server is alive");
        server.await??;
        Ok(())
    }
}
