use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

/// Response body for `GET /api/account`.
///
/// Deliberately minimal today (email only). Add fields here — verified status,
/// last-password-change, connected identities — as the account-settings surface
/// grows, rather than expanding the `/api/profile` contract, which is meant to
/// remain the public/anonymous-facing view.
#[derive(Serialize)]
pub struct AccountResponse {
    pub email: String,
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<AccountResponse>, AppError> {
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user.user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    Ok(Json(AccountResponse { email }))
}
