//! The gate of the game servers. The admin keeps one member list for all of
//! their servers, a member asks for a one time join code at the click on a
//! join button, and the plugin in the game server trades the code for the
//! username behind it. Only the servers know the gate secret, so only they
//! can trade a code.

use std::{env, sync::Arc};

use blackforge_api::{
    competitive::{Progress, Rules, Tiers},
    gate::{AddMember, CODE_SECONDS, JoinCode, Member, Verified, Verify},
};
use constant_time_eq::constant_time_eq;
use hilen_server::tracing::{info, warn};
use hilen_server::{
    AppError,
    auth::User,
    axum::{
        Extension, Json, Router,
        extract::{Path, State},
        http::{HeaderMap, header::AUTHORIZATION},
        routing::{delete, get, post},
    },
};
use sqlx::{PgPool, types::Uuid};

use crate::{
    routes::{require_admin, require_username, user_named},
    tiers,
};

/// The shared secret of the game servers, from `BLACKFORGE_GATE_SECRET`.
/// Without it no code is ever traded and nobody gets onto a server.
#[derive(Clone)]
struct GateSecret(Option<Arc<str>>);

pub fn routes(tiers: Tiers) -> Router<PgPool> {
    let secret = env::var("BLACKFORGE_GATE_SECRET")
        .ok()
        .filter(|secret| !secret.is_empty())
        .map(Arc::from);
    if secret.is_none() {
        warn!("BLACKFORGE_GATE_SECRET is not set, every join is refused");
    }
    Router::new()
        .route("/api/members", get(members).post(add_member))
        .route("/api/members/{username}", delete(remove_member))
        .route("/api/servers/{id}/join", post(join))
        .route("/api/gate/verify", post(verify))
        .route("/api/gate/progress", post(progress))
        .layer(Extension(GateSecret(secret)))
        .layer(Extension(Arc::new(tiers)))
}

async fn member_list(db: &PgPool, owner: Uuid) -> Result<Vec<Member>, AppError> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        r"SELECT p.username, u.picture FROM server_members m
JOIN profiles p ON p.user_id = m.member_id
JOIN users u ON u.id = m.member_id
WHERE m.owner_id = $1 ORDER BY lower(p.username)",
    )
    .bind(owner)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(username, picture)| Member { username, picture })
        .collect())
}

async fn members(user: User, State(db): State<PgPool>) -> Result<Json<Vec<Member>>, AppError> {
    require_admin(&db, &user, "keeps members").await?;
    Ok(Json(member_list(&db, user.id).await?))
}

async fn add_member(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<AddMember>,
) -> Result<Json<Vec<Member>>, AppError> {
    require_admin(&db, &user, "adds members").await?;
    let member = user_named(&db, &body.username).await?;
    if member == user.id {
        return Err(AppError::BadRequest(
            "the owner is always let in, no need to add yourself".to_owned(),
        ));
    }
    sqlx::query(
        "INSERT INTO server_members (owner_id, member_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user.id)
    .bind(member)
    .execute(&db)
    .await?;
    Ok(Json(member_list(&db, user.id).await?))
}

async fn remove_member(
    user: User,
    State(db): State<PgPool>,
    Path(username): Path<String>,
) -> Result<Json<Vec<Member>>, AppError> {
    require_admin(&db, &user, "removes members").await?;
    let member = user_named(&db, &username).await?;
    sqlx::query("DELETE FROM server_members WHERE owner_id = $1 AND member_id = $2")
        .bind(user.id)
        .bind(member)
        .execute(&db)
        .await?;
    // A code already handed out must not outlive the membership.
    sqlx::query(
        r"DELETE FROM join_codes c USING servers s
WHERE c.server_id = s.id AND s.owner_id = $1 AND c.user_id = $2",
    )
    .bind(user.id)
    .bind(member)
    .execute(&db)
    .await?;
    Ok(Json(member_list(&db, user.id).await?))
}

/// The server row a join needs besides the check.
type JoinRow = (String, String, String, bool, Option<String>, Vec<String>);

async fn join(
    user: User,
    Extension(tiers): Extension<Arc<Tiers>>,
    State(db): State<PgPool>,
    Path(id): Path<String>,
) -> Result<Json<JoinCode>, AppError> {
    require_username(&db, &user).await?;
    let id = Uuid::parse_str(&id).map_err(|_| AppError::NotFound)?;
    // The owner of the server or a member of the owner's list. The trade of
    // the code runs the same check again.
    let allowed: Option<JoinRow> = sqlx::query_as(
        r"SELECT s.name, p.username, s.game, s.competitive, s.world, s.boss_keys
FROM servers s JOIN profiles p ON p.user_id = s.owner_id
WHERE s.id = $1 AND (s.owner_id = $2 OR EXISTS (
    SELECT 1 FROM server_members m WHERE m.owner_id = s.owner_id AND m.member_id = $2))",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&db)
    .await?;
    let Some((name, owner, game, competitive, world, dead)) = allowed else {
        let exists: Option<(String, String)> = sqlx::query_as(
            r"SELECT s.name, p.username FROM servers s JOIN profiles p ON p.user_id = s.owner_id
WHERE s.id = $1",
        )
        .bind(id)
        .fetch_optional(&db)
        .await?;
        let (name, owner) = exists.ok_or(AppError::NotFound)?;
        return Err(AppError::BadRequest(format!(
            "you are not a member of {name}, ask {owner} to add you"
        )));
    };
    info!("join code for {name} of {owner}");
    sqlx::query("DELETE FROM join_codes WHERE expires_at < now()")
        .execute(&db)
        .await?;
    // Two random uuids are 244 random bits, only their hash is stored.
    let (code,): (String,) = sqlx::query_as(
        r"WITH made AS (
    SELECT replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', '') AS code
), stored AS (
    INSERT INTO join_codes (code_hash, user_id, server_id, expires_at)
    SELECT sha256(convert_to(code, 'UTF8')), $1, $2, now() + $3 * interval '1 second'
    FROM made
)
SELECT code FROM made",
    )
    .bind(user.id)
    .bind(id)
    .bind(CODE_SECONDS)
    .fetch_one(&db)
    .await?;
    let Rules {
        competitive,
        forbidden,
    } = tiers::rules(&tiers, &game, competitive, &dead);
    Ok(Json(JoinCode {
        code,
        competitive,
        world,
        forbidden,
    }))
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Only a game server knows the secret.
fn check_secret(secret: &GateSecret, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(secret) = &secret.0 else {
        return Err(AppError::Forbidden);
    };
    let given = bearer(headers).ok_or(AppError::Unauthorized)?;
    if !constant_time_eq(given.as_bytes(), secret.as_bytes()) {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

/// A game server tells its world and its dead bosses, and learns what it
/// forbids. It sends this at the world load, after a boss dies, and every few
/// minutes, so a change of the flag or the tiers reaches it without a restart.
async fn progress(
    Extension(secret): Extension<GateSecret>,
    Extension(tiers): Extension<Arc<Tiers>>,
    State(db): State<PgPool>,
    headers: HeaderMap,
    Json(body): Json<Progress>,
) -> Result<Json<Rules>, AppError> {
    check_secret(&secret, &headers)?;
    let row: Option<(String, bool)> = sqlx::query_as(
        r"UPDATE servers SET world = $2, boss_keys = $3, progress_at = now()
WHERE name = $1 RETURNING game, competitive",
    )
    .bind(&body.server)
    .bind(&body.world)
    .bind(&body.keys)
    .fetch_optional(&db)
    .await?;
    let (game, competitive) = row.ok_or(AppError::NotFound)?;
    Ok(Json(tiers::rules(&tiers, &game, competitive, &body.keys)))
}

async fn verify(
    Extension(secret): Extension<GateSecret>,
    State(db): State<PgPool>,
    headers: HeaderMap,
    Json(body): Json<Verify>,
) -> Result<Json<Verified>, AppError> {
    check_secret(&secret, &headers)?;
    // The delete runs whatever the checks after it say, a code is used up by
    // its first try.
    let row: Option<(String,)> = sqlx::query_as(
        r"WITH used AS (
    DELETE FROM join_codes WHERE code_hash = sha256(convert_to($1, 'UTF8'))
    RETURNING user_id, server_id, expires_at
)
SELECT p.username FROM used c
JOIN servers s ON s.id = c.server_id
JOIN profiles p ON p.user_id = c.user_id
WHERE c.expires_at > now() AND s.name = $2 AND (s.owner_id = c.user_id OR EXISTS (
    SELECT 1 FROM server_members m WHERE m.owner_id = s.owner_id AND m.member_id = c.user_id))",
    )
    .bind(&body.code)
    .bind(&body.server)
    .fetch_optional(&db)
    .await?;
    let Some((username,)) = row else {
        info!("a join code for {} was refused", body.server);
        return Err(AppError::Forbidden);
    };
    info!("{username} let onto {}", body.server);
    Ok(Json(Verified { username }))
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use hilen_server::axum::{http::StatusCode, serve};
    use reqwest::Client;
    use sqlx::postgres::PgPoolOptions;
    use tokio::{net::TcpListener, spawn, sync::oneshot};

    use super::routes;
    use blackforge_api::{
        competitive::{Progress, Tiers},
        gate::Verify,
    };

    #[tokio::test]
    /// The member routes want a login, the trade of a code wants the gate
    /// secret, and neither ever reaches the database here.
    async fn gate_routes_refuse_anonymous_requests() -> Result<()> {
        let db = PgPoolOptions::new().connect_lazy("postgres://localhost/unused")?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, stopped) = oneshot::channel();
        let server = spawn(async move {
            serve(listener, routes(Tiers::default()).with_state(db))
                .with_graceful_shutdown(async {
                    stopped.await.expect("test sends shutdown");
                })
                .await
        });
        let client = Client::new();
        let base = format!("http://{address}");
        assert_eq!(
            client
                .get(format!("{base}/api/members"))
                .send()
                .await?
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .post(format!(
                    "{base}/api/servers/8d5f1d3e-0d5f-4d1e-9d5e-1d5f1d3e0d5f/join"
                ))
                .send()
                .await?
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let verify = client
            .post(format!("{base}/api/gate/verify"))
            .header("Authorization", "Bearer wrong")
            .json(&Verify {
                server: "Durka".to_owned(),
                code: "x".to_owned(),
            })
            .send()
            .await?
            .status();
        // No secret in the test env, so the route refuses before any check.
        assert_eq!(verify, StatusCode::FORBIDDEN);
        let progress = client
            .post(format!("{base}/api/gate/progress"))
            .header("Authorization", "Bearer wrong")
            .json(&Progress {
                server: "Arkham Asylum".to_owned(),
                world: "1".to_owned(),
                keys: Vec::new(),
            })
            .send()
            .await?
            .status();
        assert_eq!(progress, StatusCode::FORBIDDEN);
        stop.send(()).expect("server is alive");
        server.await??;
        Ok(())
    }
}
