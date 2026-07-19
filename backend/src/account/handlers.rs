use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

/// Response body for `GET /api/account`.
///
/// `email` is nullable because registration doesn't require one until #83
/// (verification flow) lands. Add fields here — verified status,
/// last-password-change, connected identities — as the account-settings
/// surface grows, rather than expanding the `/api/profile` contract, which
/// remains the public/anonymous-facing view.
#[derive(Serialize)]
pub struct AccountResponse {
    pub email: Option<String>,
}

pub async fn me(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<AccountResponse>, AppError> {
    // Outer Option = row-exists?; inner Option = column-nullable. Users that
    // registered without an email round-trip as `null` in the JSON body.
    let email: Option<String> =
        sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = $1")
            .bind(user.user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    Ok(Json(AccountResponse { email }))
}
