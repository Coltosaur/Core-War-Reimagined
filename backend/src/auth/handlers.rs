use crate::auth::jwt::{encode_access_token, generate_refresh_token, hash_refresh_token};
use crate::auth::password::{hash_password, verify_password};
use crate::errors::AppError;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: String,
    pub username: String,
}

fn validate_username(username: &str) -> Result<(), AppError> {
    if username.len() < 3 || username.len() > 32 {
        return Err(AppError::BadRequest(
            "Username must be between 3 and 32 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::BadRequest(
            "Username must contain only alphanumeric characters and underscores".into(),
        ));
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<(), AppError> {
    if email.len() > 254 {
        return Err(AppError::BadRequest("Email is too long".into()));
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() || !parts[1].contains('.') {
        return Err(AppError::BadRequest("Invalid email format".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

fn build_access_cookie(token: String) -> Cookie<'static> {
    Cookie::build(("access_token", token))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(time::Duration::seconds(900))
        .build()
}

fn build_refresh_cookie(token: String) -> Cookie<'static> {
    Cookie::build(("refresh_token", token))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/auth/refresh")
        .max_age(time::Duration::seconds(604800))
        .build()
}

fn clear_access_cookie() -> Cookie<'static> {
    Cookie::build(("access_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(time::Duration::ZERO)
        .build()
}

fn clear_refresh_cookie() -> Cookie<'static> {
    Cookie::build(("refresh_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/auth/refresh")
        .max_age(time::Duration::ZERO)
        .build()
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    validate_username(&body.username)?;
    validate_email(&body.email)?;
    validate_password(&body.password)?;

    let password_hash = hash_password(&body.password)?;

    let row = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id, username",
    )
    .bind(&body.username)
    .bind(&body.email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.constraint().is_some() => {
            let constraint = db_err.constraint().unwrap();
            if constraint.contains("username") {
                AppError::Conflict("Username is already taken".into())
            } else if constraint.contains("email") {
                AppError::Conflict("Email is already registered".into())
            } else {
                AppError::Internal(e.to_string())
            }
        }
        _ => AppError::Internal(e.to_string()),
    })?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            user_id: row.0.to_string(),
            username: row.1,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<AuthResponse>), AppError> {
    let user = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
        "SELECT id, username, password_hash FROM users WHERE username = $1 OR email = $1",
    )
    .bind(&body.username_or_email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid credentials".into()))?;

    let (user_id, username, password_hash) = user;

    if !verify_password(&body.password, &password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    let access_token = encode_access_token(user_id, &username, &state.config.jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(7);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(&state.db)
        .await?;

    let jar = jar
        .add(build_access_cookie(access_token))
        .add(build_refresh_cookie(refresh_token));

    Ok((
        jar,
        Json(AuthResponse {
            user_id: user_id.to_string(),
            username,
        }),
    ))
}

pub async fn refresh(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Json<AuthResponse>), AppError> {
    let refresh_token = jar
        .get("refresh_token")
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("Missing refresh token".into()))?;

    let token_hash = hash_refresh_token(&refresh_token);

    let row = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid)>(
        "DELETE FROM refresh_tokens WHERE token_hash = $1 AND expires_at > now() RETURNING id, user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid or expired refresh token".into()))?;

    let user_id = row.1;

    let user = sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    let username = user.0;

    let new_access_token = encode_access_token(user_id, &username, &state.config.jwt_secret)?;
    let new_refresh_token = generate_refresh_token();
    let new_token_hash = hash_refresh_token(&new_refresh_token);
    let expires_at = Utc::now() + Duration::days(7);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&new_token_hash)
        .bind(expires_at)
        .execute(&state.db)
        .await?;

    let jar = jar
        .add(build_access_cookie(new_access_token))
        .add(build_refresh_cookie(new_refresh_token));

    Ok((
        jar,
        Json(AuthResponse {
            user_id: user_id.to_string(),
            username,
        }),
    ))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    if let Some(refresh_token) = jar.get("refresh_token").map(|c| c.value().to_string()) {
        let token_hash = hash_refresh_token(&refresh_token);
        sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(&state.db)
            .await?;
    }

    let jar = jar.add(clear_access_cookie()).add(clear_refresh_cookie());

    Ok((jar, StatusCode::NO_CONTENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_username() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("bob_123").is_ok());
        assert!(validate_username("a_b").is_ok());
    }

    #[test]
    fn username_too_short() {
        assert!(validate_username("ab").is_err());
    }

    #[test]
    fn username_too_long() {
        let long = "a".repeat(33);
        assert!(validate_username(&long).is_err());
    }

    #[test]
    fn username_invalid_chars() {
        assert!(validate_username("alice!").is_err());
        assert!(validate_username("bob smith").is_err());
        assert!(validate_username("user@name").is_err());
    }

    #[test]
    fn valid_email() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("a@b.co").is_ok());
    }

    #[test]
    fn email_missing_at() {
        assert!(validate_email("userexample.com").is_err());
    }

    #[test]
    fn email_missing_dot_in_domain() {
        assert!(validate_email("user@localhost").is_err());
    }

    #[test]
    fn email_empty_parts() {
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("user@").is_err());
    }

    #[test]
    fn email_too_long() {
        let long = format!("{}@example.com", "a".repeat(250));
        assert!(validate_email(&long).is_err());
    }

    #[test]
    fn valid_password() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password("a-very-long-password-indeed").is_ok());
    }

    #[test]
    fn password_too_short() {
        assert!(validate_password("1234567").is_err());
    }

    #[test]
    fn access_cookie_properties() {
        let cookie = build_access_cookie("tok".into());
        assert_eq!(cookie.name(), "access_token");
        assert_eq!(cookie.value(), "tok");
        assert!(cookie.http_only().unwrap());
        assert!(cookie.secure().unwrap());
        assert_eq!(cookie.same_site().unwrap(), SameSite::Strict);
        assert_eq!(cookie.path().unwrap(), "/");
        assert_eq!(cookie.max_age().unwrap(), time::Duration::seconds(900));
    }

    #[test]
    fn refresh_cookie_properties() {
        let cookie = build_refresh_cookie("ref".into());
        assert_eq!(cookie.name(), "refresh_token");
        assert_eq!(cookie.path().unwrap(), "/api/auth/refresh");
        assert_eq!(cookie.max_age().unwrap(), time::Duration::seconds(604800));
    }

    #[test]
    fn clear_cookies_have_zero_max_age() {
        let ac = clear_access_cookie();
        let rc = clear_refresh_cookie();
        assert_eq!(ac.max_age().unwrap(), time::Duration::ZERO);
        assert_eq!(rc.max_age().unwrap(), time::Duration::ZERO);
    }
}
