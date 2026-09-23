//! Registered game servers. Anybody reads the whole list, no login needed,
//! only the owner changes a row. A change of a server that is not mine gets the
//! same 404 as one that does not exist. Only `JOIN_ADMIN` sets a join address.

use blackforge_api::servers::{
    JOIN_ADMIN, SaveServer, Server, ServerMod, normalize_address, normalize_name, validate,
};
use hilen_server::{
    AppError,
    auth::User,
    axum::{
        Json, Router,
        extract::{Path, State},
        routing::{get, put},
    },
};
use serde_json::{from_str, to_string};
use sqlx::{PgPool, types::Uuid};

use crate::routes::{require_username, username_of};

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/api/servers", get(list).post(create))
        .route("/api/servers/{id}", put(update).delete(delete))
}

type Row = (
    Uuid,
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
);

// The two reads spell the columns out twice, sqlx takes only a literal query.
const LIST: &str = r"
SELECT s.id, s.name, s.game, p.username, s.mods, EXTRACT(EPOCH FROM s.updated_at)::bigint,
    EXTRACT(EPOCH FROM s.created_at)::bigint, s.address
FROM servers s JOIN profiles p ON p.user_id = s.owner_id
ORDER BY lower(s.name), s.created_at";

const ONE: &str = r"
SELECT s.id, s.name, s.game, p.username, s.mods, EXTRACT(EPOCH FROM s.updated_at)::bigint,
    EXTRACT(EPOCH FROM s.created_at)::bigint, s.address
FROM servers s JOIN profiles p ON p.user_id = s.owner_id
WHERE s.id = $1";

fn server_of(row: Row) -> Result<Server, AppError> {
    let (id, name, game, owner, mods, updated, created, address) = row;
    let mods: Vec<ServerMod> = from_str(&mods).map_err(|error| AppError::Internal(error.into()))?;
    Ok(Server {
        id: id.to_string(),
        name,
        game,
        owner,
        mods,
        updated,
        created,
        address,
    })
}

/// The one route without a `User`, a player picks a server and installs its
/// mods before ever signing in.
async fn list(State(db): State<PgPool>) -> Result<Json<Vec<Server>>, AppError> {
    let rows: Vec<Row> = sqlx::query_as(LIST).fetch_all(&db).await?;
    rows.into_iter()
        .map(server_of)
        .collect::<Result<_, _>>()
        .map(Json)
}

async fn one(db: &PgPool, id: Uuid) -> Result<Server, AppError> {
    let row: Option<Row> = sqlx::query_as(ONE).bind(id).fetch_optional(db).await?;
    server_of(row.ok_or(AppError::NotFound)?)
}

/// The id of the path. It comes as text because `Path<Uuid>` needs the serde
/// feature of uuid, which is on only through other workspace crates, and the
/// Dockerfile builds this crate alone. A malformed id reads like an unknown one.
fn server_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id).map_err(|_| AppError::NotFound)
}

/// The checked body as it goes into the table.
struct Checked {
    name: String,
    game: String,
    mods: String,
    address: AddressChange,
}

/// What a save does to the stored join address.
enum AddressChange {
    /// No field, an app before the join address sent the save.
    Keep,
    Remove,
    Set(String),
}

impl AddressChange {
    fn changes(&self) -> bool {
        !matches!(self, Self::Keep)
    }

    fn stored(&self) -> Option<&str> {
        match self {
            Self::Set(address) => Some(address),
            Self::Keep | Self::Remove => None,
        }
    }
}

fn checked(body: &SaveServer) -> Result<Checked, AppError> {
    validate(body).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let name =
        normalize_name(&body.name).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let game = body.game.trim();
    if game.is_empty() {
        return Err(AppError::BadRequest("a server needs a game".to_owned()));
    }
    let mods = to_string(&body.mods).map_err(|error| AppError::Internal(error.into()))?;
    let address = match body.address.as_deref().map(normalize_address) {
        None => AddressChange::Keep,
        Some(Ok(None)) => AddressChange::Remove,
        Some(Ok(Some(address))) => AddressChange::Set(address),
        Some(Err(error)) => return Err(AppError::BadRequest(error.to_string())),
    };
    Ok(Checked {
        name,
        game: game.to_owned(),
        mods,
        address,
    })
}

/// A join address puts a button in every player's game menu, so only the
/// join admin sets one.
async fn allow_address(db: &PgPool, user: &User, body: &Checked) -> Result<(), AppError> {
    if body.address.stored().is_none() {
        return Ok(());
    }
    if username_of(db, user.id).await?.as_deref() == Some(JOIN_ADMIN) {
        return Ok(());
    }
    Err(AppError::BadRequest(
        "only the join admin gives a server a join address".to_owned(),
    ))
}

/// A name belongs to one server of a game, a pin names its server.
fn taken(error: sqlx::Error, name: &str) -> AppError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => {
            AppError::BadRequest(format!("a server called {name} already exists"))
        }
        _ => error.into(),
    }
}

async fn create(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<SaveServer>,
) -> Result<Json<Server>, AppError> {
    require_username(&db, &user).await?;
    let body = checked(&body)?;
    allow_address(&db, &user, &body).await?;
    let (id,): (Uuid,) = sqlx::query_as(
        r"INSERT INTO servers (owner_id, game, name, mods, address) VALUES ($1, $2, $3, $4, $5)
RETURNING id",
    )
    .bind(user.id)
    .bind(&body.game)
    .bind(&body.name)
    .bind(&body.mods)
    .bind(body.address.stored())
    .fetch_one(&db)
    .await
    .map_err(|error| taken(error, &body.name))?;
    Ok(Json(one(&db, id).await?))
}

async fn update(
    user: User,
    State(db): State<PgPool>,
    Path(id): Path<String>,
    Json(body): Json<SaveServer>,
) -> Result<Json<Server>, AppError> {
    let id = server_id(&id)?;
    let body = checked(&body)?;
    allow_address(&db, &user, &body).await?;
    let current: Option<(String,)> =
        sqlx::query_as("SELECT name FROM servers WHERE id = $1 AND owner_id = $2")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&db)
            .await?;
    let Some((current,)) = current else {
        return Err(AppError::NotFound);
    };
    // Players' pins name the server, a new name would leave them behind.
    if current != body.name {
        return Err(AppError::BadRequest(
            "a server keeps the name it was registered with".to_owned(),
        ));
    }
    sqlx::query(
        r"UPDATE servers SET game = $3, mods = $4, updated_at = now(),
address = CASE WHEN $5 THEN $6 ELSE address END
WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .bind(&body.game)
    .bind(&body.mods)
    .bind(body.address.changes())
    .bind(body.address.stored())
    .execute(&db)
    .await?;
    Ok(Json(one(&db, id).await?))
}

async fn delete(
    user: User,
    State(db): State<PgPool>,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    let id = server_id(&id)?;
    let done = sqlx::query("DELETE FROM servers WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
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
    /// The list itself is public, so only the writes are checked here.
    async fn server_routes_refuse_anonymous_writes() -> Result<()> {
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
        let base = format!("http://{address}/api/servers");
        let one = format!("{base}/8d5f1d3e-0d5f-4d1e-9d5e-1d5f1d3e0d5f");
        assert_eq!(
            client.post(&base).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client.put(&one).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client.delete(&one).send().await?.status(),
            StatusCode::UNAUTHORIZED
        );
        stop.send(()).expect("server is alive");
        server.await??;
        Ok(())
    }
}
