use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::models::warrior::Warrior;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_NAME_LEN: usize = 64;
const MAX_SOURCE_LEN: usize = 64_000;
const MAX_PAGE_SIZE: i64 = 100;
const DEFAULT_PAGE_SIZE: i64 = 50;

#[derive(Deserialize)]
pub struct CreateWarriorRequest {
    pub name: String,
    pub source: String,
}

#[derive(Deserialize)]
pub struct UpdateWarriorRequest {
    pub name: Option<String>,
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Serialize)]
pub struct WarriorListResponse {
    pub warriors: Vec<Warrior>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

fn validate_name(name: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Warrior name cannot be empty".into()));
    }
    if name.len() > MAX_NAME_LEN {
        return Err(AppError::BadRequest(format!(
            "Warrior name must not exceed {MAX_NAME_LEN} characters"
        )));
    }
    Ok(())
}

fn validate_source(source: &str) -> Result<(), AppError> {
    if source.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Warrior source cannot be empty".into(),
        ));
    }
    if source.len() > MAX_SOURCE_LEN {
        return Err(AppError::BadRequest(format!(
            "Warrior source must not exceed {MAX_SOURCE_LEN} characters"
        )));
    }
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateWarriorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = body.name.trim().to_string();
    let source = body.source.clone();

    validate_name(&name)?;
    validate_source(&source)?;

    let warrior = sqlx::query_as::<_, Warrior>(
        "INSERT INTO warriors (user_id, name, source) VALUES ($1, $2, $3) \
         RETURNING id, user_id, name, source, created_at, updated_at",
    )
    .bind(user.user_id)
    .bind(&name)
    .bind(&source)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(warrior)))
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Warrior>, AppError> {
    let warrior = sqlx::query_as::<_, Warrior>(
        "SELECT id, user_id, name, source, created_at, updated_at \
         FROM warriors WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Warrior not found".into()))?;

    Ok(Json(warrior))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<WarriorListResponse>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query
        .per_page
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * per_page;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM warriors WHERE user_id = $1")
        .bind(user.user_id)
        .fetch_one(&state.db)
        .await?;

    let warriors = sqlx::query_as::<_, Warrior>(
        "SELECT id, user_id, name, source, created_at, updated_at \
         FROM warriors WHERE user_id = $1 \
         ORDER BY updated_at DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(user.user_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(WarriorListResponse {
        warriors,
        total,
        page,
        per_page,
    }))
}

pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateWarriorRequest>,
) -> Result<Json<Warrior>, AppError> {
    let existing = sqlx::query_as::<_, Warrior>(
        "SELECT id, user_id, name, source, created_at, updated_at \
         FROM warriors WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Warrior not found".into()))?;

    let name = match &body.name {
        Some(n) => {
            validate_name(n)?;
            n.trim().to_string()
        }
        None => existing.name,
    };

    let source = match &body.source {
        Some(s) => {
            validate_source(s)?;
            s.clone()
        }
        None => existing.source,
    };

    let warrior = sqlx::query_as::<_, Warrior>(
        "UPDATE warriors SET name = $1, source = $2, updated_at = now() \
         WHERE id = $3 AND user_id = $4 \
         RETURNING id, user_id, name, source, created_at, updated_at",
    )
    .bind(&name)
    .bind(&source)
    .bind(id)
    .bind(user.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(warrior))
}

pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM warriors WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.user_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Warrior not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_name() {
        assert!(validate_name("Imp").is_ok());
        assert!(validate_name("My Warrior 2").is_ok());
        assert!(validate_name(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn name_empty() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn name_too_long() {
        assert!(validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_source() {
        assert!(validate_source("MOV.I $0, $1").is_ok());
    }

    #[test]
    fn source_empty() {
        assert!(validate_source("").is_err());
        assert!(validate_source("   ").is_err());
    }

    #[test]
    fn source_too_long() {
        assert!(validate_source(&"a".repeat(64_001)).is_err());
    }

    #[test]
    fn source_at_max_length() {
        assert!(validate_source(&"a".repeat(64_000)).is_ok());
    }
}
