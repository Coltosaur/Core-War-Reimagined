use crate::auth::middleware::{AuthUser, OptionalAuthUser};
use crate::errors::AppError;
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ProfileResponse {
    pub user_id: String,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub warrior_count: i64,
    pub match_count: i64,
    pub wins: i64,
    pub losses: i64,
    pub ties: i64,
}

#[derive(Serialize)]
pub struct PublicWarrior {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct PublicProfileResponse {
    pub user_id: String,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub warrior_count: i64,
    pub match_count: i64,
    pub wins: i64,
    pub losses: i64,
    pub ties: i64,
    pub warriors: Vec<PublicWarrior>,
}

async fn fetch_match_stats(db: &sqlx::PgPool, user_id: Uuid) -> (i64, i64, i64, i64) {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT \
           COUNT(*), \
           COUNT(*) FILTER (WHERE (red_user_id = $1 AND result = 'red_win') \
                                OR (blue_user_id = $1 AND result = 'blue_win')), \
           COUNT(*) FILTER (WHERE (red_user_id = $1 AND result = 'blue_win') \
                                OR (blue_user_id = $1 AND result = 'red_win')), \
           COUNT(*) FILTER (WHERE result IN ('tie', 'all_dead')) \
         FROM matches WHERE red_user_id = $1 OR blue_user_id = $1",
    )
    .bind(user_id)
    .fetch_one(db)
    .await;

    row.unwrap_or_default()
}

async fn build_profile(
    db: &sqlx::PgPool,
    user_id: Uuid,
    username: &str,
) -> Result<ProfileResponse, AppError> {
    let created_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT created_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await?;

    let warrior_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM warriors WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await?;

    let (match_count, wins, losses, ties) = fetch_match_stats(db, user_id).await;

    Ok(ProfileResponse {
        user_id: user_id.to_string(),
        username: username.to_string(),
        created_at,
        warrior_count,
        match_count,
        wins,
        losses,
        ties,
    })
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<ProfileResponse>, AppError> {
    let profile = build_profile(&state.db, user.user_id, &user.username).await?;
    Ok(Json(profile))
}

pub async fn public_profile(
    State(state): State<AppState>,
    _auth: OptionalAuthUser,
    Path(username): Path<String>,
) -> Result<Json<PublicProfileResponse>, AppError> {
    let row = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>)>(
        "SELECT id, username, created_at FROM users WHERE username = $1",
    )
    .bind(&username)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    let (user_id, username, created_at) = row;

    let profile = build_profile(&state.db, user_id, &username).await?;

    let warriors = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>, DateTime<Utc>)>(
        "SELECT id, name, created_at, updated_at FROM warriors \
         WHERE user_id = $1 ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(id, name, ca, ua)| PublicWarrior {
        id,
        name,
        created_at: ca,
        updated_at: ua,
    })
    .collect();

    Ok(Json(PublicProfileResponse {
        user_id: profile.user_id,
        username: profile.username,
        created_at,
        warrior_count: profile.warrior_count,
        match_count: profile.match_count,
        wins: profile.wins,
        losses: profile.losses,
        ties: profile.ties,
        warriors,
    }))
}
