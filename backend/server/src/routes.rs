//! The social routes. Every one takes a `User`, so a request with no live
//! session never gets past the extractor.

use blackforge_api::{
    Friend, FriendName, Friends, Me, SetUsername, SharedProfile, Status, username,
};
use hilen_server::{
    AppError,
    auth::User,
    axum::{
        Json, Router,
        extract::{Path, State},
        routing::{get, post, put},
    },
};
use serde_json::{from_str, to_string};
use sqlx::{PgPool, types::Uuid};

/// Silence for this long reads as not in game, the app reports once a minute.
const IN_GAME_SECONDS: f64 = 120.0;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/api/me", get(me))
        .route("/api/me/username", post(set_username))
        .route("/api/friends", get(friends))
        .route("/api/friends/request", post(request))
        .route("/api/friends/accept", post(accept))
        .route("/api/friends/decline", post(decline))
        .route("/api/friends/remove", post(remove))
        .route("/api/friends/{username}/profile", get(friend_profile))
        .route("/api/profile", put(put_profile))
        .route("/api/status", post(set_status))
}

async fn username_of(db: &PgPool, user_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT username FROM profiles WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(username,)| username))
}

/// Friends only exist between people with a username, it is all they see of
/// each other.
async fn require_username(db: &PgPool, user: &User) -> Result<(), AppError> {
    match username_of(db, user.id).await? {
        Some(_) => Ok(()),
        None => Err(AppError::BadRequest("pick a username first".to_owned())),
    }
}

/// The user behind a name somebody typed.
async fn user_named(db: &PgPool, name: &str) -> Result<Uuid, AppError> {
    let name =
        username::normalize(name).map_err(|error| AppError::BadRequest(error.to_string()))?;
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT user_id FROM profiles WHERE username = $1")
        .bind(&name)
        .fetch_optional(db)
        .await?;
    row.map(|(id,)| id)
        .ok_or_else(|| AppError::BadRequest(format!("nobody is called {name}")))
}

fn ordered(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    if a < b { (a, b) } else { (b, a) }
}

async fn are_friends(db: &PgPool, a: Uuid, b: Uuid) -> Result<bool, sqlx::Error> {
    let (first, second) = ordered(a, b);
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM friendships WHERE user_a = $1 AND user_b = $2")
            .bind(first)
            .bind(second)
            .fetch_optional(db)
            .await?;
    Ok(row.is_some())
}

/// Both directions of a request go away with the friendship they turn into.
async fn befriend(db: &PgPool, a: Uuid, b: Uuid) -> Result<(), sqlx::Error> {
    let (first, second) = ordered(a, b);
    let mut transaction = db.begin().await?;

    sqlx::query("INSERT INTO friendships (user_a, user_b) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(first)
        .bind(second)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        r"
DELETE FROM friend_requests
WHERE (from_user = $1 AND to_user = $2) OR (from_user = $2 AND to_user = $1)",
    )
    .bind(a)
    .bind(b)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await
}

async fn me(user: User, State(db): State<PgPool>) -> Result<Json<Me>, AppError> {
    Ok(Json(Me {
        username: username_of(&db, user.id).await?,
    }))
}

async fn set_username(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<SetUsername>,
) -> Result<Json<Me>, AppError> {
    let name = username::normalize(&body.username)
        .map_err(|error| AppError::BadRequest(error.to_string()))?;

    if username_of(&db, user.id).await?.is_some() {
        return Err(AppError::BadRequest(
            "a username cannot be changed".to_owned(),
        ));
    }

    let inserted = sqlx::query("INSERT INTO profiles (user_id, username) VALUES ($1, $2)")
        .bind(user.id)
        .bind(&name)
        .execute(&db)
        .await;

    match inserted {
        Ok(_) => Ok(Json(Me {
            username: Some(name),
        })),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Err(
            AppError::BadRequest(format!("the username {name} is taken")),
        ),
        Err(error) => Err(error.into()),
    }
}

async fn friends(user: User, State(db): State<PgPool>) -> Result<Json<Friends>, AppError> {
    let rows: Vec<(String, bool)> = sqlx::query_as(
        r"
SELECT p.username,
       COALESCE(s.in_game AND s.last_seen > now() - make_interval(secs => $2), false)
FROM friendships f
JOIN profiles p ON p.user_id = CASE WHEN f.user_a = $1 THEN f.user_b ELSE f.user_a END
LEFT JOIN game_status s ON s.user_id = p.user_id
WHERE f.user_a = $1 OR f.user_b = $1
ORDER BY p.username",
    )
    .bind(user.id)
    .bind(IN_GAME_SECONDS)
    .fetch_all(&db)
    .await?;

    let incoming: Vec<(String,)> = sqlx::query_as(
        r"
SELECT p.username FROM friend_requests r
JOIN profiles p ON p.user_id = r.from_user
WHERE r.to_user = $1
ORDER BY r.created_at",
    )
    .bind(user.id)
    .fetch_all(&db)
    .await?;

    let outgoing: Vec<(String,)> = sqlx::query_as(
        r"
SELECT p.username FROM friend_requests r
JOIN profiles p ON p.user_id = r.to_user
WHERE r.from_user = $1
ORDER BY r.created_at",
    )
    .bind(user.id)
    .fetch_all(&db)
    .await?;

    Ok(Json(Friends {
        friends: rows
            .into_iter()
            .map(|(username, in_game)| Friend { username, in_game })
            .collect(),
        incoming: incoming.into_iter().map(|(name,)| name).collect(),
        outgoing: outgoing.into_iter().map(|(name,)| name).collect(),
    }))
}

async fn request(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<FriendName>,
) -> Result<(), AppError> {
    require_username(&db, &user).await?;
    let other = user_named(&db, &body.username).await?;

    if other == user.id {
        return Err(AppError::BadRequest("that is you".to_owned()));
    }
    if are_friends(&db, user.id, other).await? {
        return Ok(());
    }

    // Two people who ask each other both agree already.
    let they_asked: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM friend_requests WHERE from_user = $1 AND to_user = $2")
            .bind(other)
            .bind(user.id)
            .fetch_optional(&db)
            .await?;
    if they_asked.is_some() {
        return Ok(befriend(&db, user.id, other).await?);
    }

    sqlx::query(
        "INSERT INTO friend_requests (from_user, to_user) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user.id)
    .bind(other)
    .execute(&db)
    .await?;
    Ok(())
}

async fn accept(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<FriendName>,
) -> Result<(), AppError> {
    require_username(&db, &user).await?;
    let other = user_named(&db, &body.username).await?;

    let asked: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM friend_requests WHERE from_user = $1 AND to_user = $2")
            .bind(other)
            .bind(user.id)
            .fetch_optional(&db)
            .await?;
    if asked.is_none() {
        return Err(AppError::BadRequest(format!(
            "{} did not ask",
            body.username
        )));
    }

    Ok(befriend(&db, user.id, other).await?)
}

async fn decline(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<FriendName>,
) -> Result<(), AppError> {
    let other = user_named(&db, &body.username).await?;

    sqlx::query("DELETE FROM friend_requests WHERE from_user = $1 AND to_user = $2")
        .bind(other)
        .bind(user.id)
        .execute(&db)
        .await?;
    Ok(())
}

/// Ends a friendship for both sides, and takes back a request of my own.
async fn remove(
    user: User,
    State(db): State<PgPool>,
    Json(body): Json<FriendName>,
) -> Result<(), AppError> {
    let other = user_named(&db, &body.username).await?;
    let (first, second) = ordered(user.id, other);

    sqlx::query("DELETE FROM friendships WHERE user_a = $1 AND user_b = $2")
        .bind(first)
        .bind(second)
        .execute(&db)
        .await?;
    sqlx::query("DELETE FROM friend_requests WHERE from_user = $1 AND to_user = $2")
        .bind(user.id)
        .bind(other)
        .execute(&db)
        .await?;
    Ok(())
}

async fn put_profile(
    user: User,
    State(db): State<PgPool>,
    Json(profile): Json<SharedProfile>,
) -> Result<(), AppError> {
    let json = to_string(&profile).map_err(|error| AppError::Internal(error.into()))?;

    sqlx::query(
        r"
INSERT INTO shared_profiles (user_id, profile) VALUES ($1, $2)
ON CONFLICT (user_id) DO UPDATE SET profile = EXCLUDED.profile, updated_at = now()",
    )
    .bind(user.id)
    .bind(json)
    .execute(&db)
    .await?;
    Ok(())
}

/// Only for an accepted friend. Anybody else gets the same 404 as for a name
/// that does not exist, so the route does not tell who has an account.
async fn friend_profile(
    user: User,
    State(db): State<PgPool>,
    Path(name): Path<String>,
) -> Result<Json<SharedProfile>, AppError> {
    let other = user_named(&db, &name)
        .await
        .map_err(|_| AppError::NotFound)?;
    if !are_friends(&db, user.id, other).await? {
        return Err(AppError::NotFound);
    }

    let row: Option<(String,)> =
        sqlx::query_as("SELECT profile FROM shared_profiles WHERE user_id = $1")
            .bind(other)
            .fetch_optional(&db)
            .await?;

    let profile = match row {
        Some((json,)) => from_str(&json).map_err(|error| AppError::Internal(error.into()))?,
        None => SharedProfile::default(),
    };
    Ok(Json(profile))
}

async fn set_status(
    user: User,
    State(db): State<PgPool>,
    Json(status): Json<Status>,
) -> Result<(), AppError> {
    sqlx::query(
        r"
INSERT INTO game_status (user_id, in_game) VALUES ($1, $2)
ON CONFLICT (user_id) DO UPDATE SET in_game = EXCLUDED.in_game, last_seen = now()",
    )
    .bind(user.id)
    .bind(status.in_game)
    .execute(&db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::types::Uuid;

    use super::ordered;

    #[test]
    fn a_pair_is_stored_one_way_only() {
        let small = Uuid::from_u128(1);
        let big = Uuid::from_u128(2);
        assert_eq!(ordered(small, big), (small, big));
        assert_eq!(ordered(big, small), (small, big));
    }
}
