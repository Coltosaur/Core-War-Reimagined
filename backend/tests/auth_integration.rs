use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{middleware, Router};
use core_war_backend::{auth, AppConfig, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use tower::ServiceExt;

const FRONTEND_URL: &str = "http://localhost:5173";
const JWT_SECRET: &[u8] = b"integration-test-secret-that-is-at-least-32-bytes!!";

fn test_state(pool: PgPool) -> AppState {
    AppState {
        db: pool,
        config: AppConfig {
            frontend_url: FRONTEND_URL.into(),
            jwt_secret: JWT_SECRET.to_vec(),
            trusted_proxies: vec![],
        },
    }
}

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/auth/login", post(auth::handlers::login))
        .route("/api/auth/refresh", post(auth::handlers::refresh))
        .route("/api/auth/logout", post(auth::handlers::logout))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

// --- Request builders ---

fn post_json(path: &str, body: &Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("origin", FRONTEND_URL)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post_with_cookies(path: &str, cookies: &str) -> Request<Body> {
    Request::post(path)
        .header("origin", FRONTEND_URL)
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

// --- Response helpers ---

struct TestResponse {
    status: StatusCode,
    json: Value,
    cookies: HashMap<String, String>,
    set_cookie_headers: Vec<String>,
}

fn extract_cookies(headers: &axum::http::HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for header in headers.get_all("set-cookie") {
        if let Ok(s) = header.to_str() {
            if let Some(cookie_part) = s.split(';').next() {
                if let Some((name, value)) = cookie_part.split_once('=') {
                    map.insert(name.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }
    map
}

async fn send(router: Router, req: Request<Body>) -> TestResponse {
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let cookies = extract_cookies(resp.headers());
    let set_cookie_headers: Vec<String> = resp
        .headers()
        .get_all("set-cookie")
        .into_iter()
        .filter_map(|v| v.to_str().ok().map(String::from))
        .collect();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    TestResponse {
        status,
        json,
        cookies,
        set_cookie_headers,
    }
}

// --- Data helpers ---

fn register_body(username: &str, email: &str, password: &str) -> Value {
    json!({"username": username, "email": email, "password": password})
}

fn login_body(username_or_email: &str, password: &str) -> Value {
    json!({"username_or_email": username_or_email, "password": password})
}

async fn register_and_login(
    router: &Router,
    username: &str,
    email: &str,
    password: &str,
) -> HashMap<String, String> {
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body(username, email, password),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router.clone(),
        post_json("/api/auth/login", &login_body(username, password)),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    resp.cookies
}

// =============================================================================
// Register tests
// =============================================================================

#[sqlx::test]
async fn register_success(pool: PgPool) {
    let resp = send(
        app(pool),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["username"], "alice");
    let uid: uuid::Uuid = resp.json["user_id"].as_str().unwrap().parse().unwrap();
    assert!(!uid.is_nil());
}

#[sqlx::test]
async fn register_duplicate_username(pool: PgPool) {
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice1@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice2@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CONFLICT);
    assert!(resp.json["error"].as_str().unwrap().contains("Username"));
}

#[sqlx::test]
async fn register_duplicate_email(pool: PgPool) {
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("user1", "same@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("user2", "same@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CONFLICT);
    assert!(resp.json["error"].as_str().unwrap().contains("Email"));
}

#[sqlx::test]
async fn register_validation_errors(pool: PgPool) {
    let router = app(pool);

    // Username too short
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("ab", "ab@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Username invalid chars
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("user!name", "un@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Invalid email
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("validuser", "not-an-email", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Password too short
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("validuser", "valid@example.com", "short"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Password too long
    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("validuser", "valid@example.com", &"a".repeat(1001)),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn register_missing_fields(pool: PgPool) {
    let resp = send(
        app(pool),
        post_json("/api/auth/register", &json!({"username": "test"})),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn register_email_normalized(pool: PgPool) {
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "  Alice@Example.COM  ", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    // Same email with different case should conflict
    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("bob", "alice@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn register_username_trimmed(pool: PgPool) {
    let resp = send(
        app(pool),
        post_json(
            "/api/auth/register",
            &register_body("  alice  ", "alice@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["username"], "alice");
}

// =============================================================================
// Login tests
// =============================================================================

#[sqlx::test]
async fn login_by_username(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password123")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["username"], "alice");
    assert!(resp.json["user_id"].is_string());
}

#[sqlx::test]
async fn login_by_email(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json(
            "/api/auth/login",
            &login_body("alice@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["username"], "alice");
}

#[sqlx::test]
async fn login_email_case_insensitive(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json(
            "/api/auth/login",
            &login_body("ALICE@EXAMPLE.COM", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["username"], "alice");
}

#[sqlx::test]
async fn login_wrong_password(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "wrongpass123")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(resp.json["error"], "Invalid credentials");
}

#[sqlx::test]
async fn login_nonexistent_user(pool: PgPool) {
    let resp = send(
        app(pool),
        post_json("/api/auth/login", &login_body("ghost", "password123")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(resp.json["error"], "Invalid credentials");
}

#[sqlx::test]
async fn login_sets_both_cookies(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password123")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(resp.cookies.contains_key("access_token"));
    assert!(resp.cookies.contains_key("refresh_token"));
    assert!(!resp.cookies["access_token"].is_empty());
    assert!(!resp.cookies["refresh_token"].is_empty());
}

#[sqlx::test]
async fn login_cookie_properties(pool: PgPool) {
    let router = app(pool);
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password123")),
    )
    .await;

    let access_header = resp
        .set_cookie_headers
        .iter()
        .find(|h| h.starts_with("access_token="))
        .expect("access_token Set-Cookie header missing");

    assert!(access_header.contains("HttpOnly"));
    assert!(access_header.contains("Secure"));
    assert!(access_header.contains("SameSite=Strict"));
    assert!(access_header.contains("Path=/"));
    assert!(access_header.contains("Max-Age=900"));

    let refresh_header = resp
        .set_cookie_headers
        .iter()
        .find(|h| h.starts_with("refresh_token="))
        .expect("refresh_token Set-Cookie header missing");

    assert!(refresh_header.contains("HttpOnly"));
    assert!(refresh_header.contains("Secure"));
    assert!(refresh_header.contains("Path=/api/auth/refresh"));
    assert!(refresh_header.contains("Max-Age=604800"));
}

// =============================================================================
// Refresh tests
// =============================================================================

#[sqlx::test]
async fn refresh_rotates_token(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password123").await;
    let old_refresh = cookies["refresh_token"].clone();

    let resp = send(
        router,
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={old_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["username"], "alice");
    assert!(resp.cookies.contains_key("access_token"));
    assert!(resp.cookies.contains_key("refresh_token"));
    assert_ne!(resp.cookies["refresh_token"], old_refresh);
}

#[sqlx::test]
async fn refresh_old_token_rejected(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password123").await;
    let old_refresh = cookies["refresh_token"].clone();

    // Use the token once (rotates it)
    let resp = send(
        router.clone(),
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={old_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Try the same token again — should be rejected
    let resp = send(
        router,
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={old_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn refresh_missing_cookie(pool: PgPool) {
    let resp = send(
        app(pool),
        Request::post("/api/auth/refresh")
            .header("origin", FRONTEND_URL)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    assert!(resp.json["error"]
        .as_str()
        .unwrap()
        .contains("refresh token"));
}

#[sqlx::test]
async fn refresh_expired_token(pool: PgPool) {
    let router = app(pool.clone());

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice@example.com", "password123"),
        ),
    )
    .await;
    let user_id: uuid::Uuid = resp.json["user_id"].as_str().unwrap().parse().unwrap();

    // Insert a refresh token with a past expiry directly into the DB
    let expired_token = core_war_backend::auth::jwt::generate_refresh_token();
    let token_hash = core_war_backend::auth::jwt::hash_refresh_token(&expired_token);
    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(chrono::Utc::now() - chrono::Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();

    let resp = send(
        router,
        post_with_cookies(
            "/api/auth/refresh",
            &format!("refresh_token={expired_token}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Logout tests
// =============================================================================

#[sqlx::test]
async fn logout_clears_cookies(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password123").await;
    let refresh = &cookies["refresh_token"];

    let resp = send(
        router,
        post_with_cookies("/api/auth/logout", &format!("refresh_token={refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // Cleared cookies should have empty values in Set-Cookie headers
    let access_clear = resp
        .set_cookie_headers
        .iter()
        .find(|h| h.starts_with("access_token="))
        .expect("access_token clear cookie missing");
    assert!(access_clear.contains("Max-Age=0"));

    let refresh_clear = resp
        .set_cookie_headers
        .iter()
        .find(|h| h.starts_with("refresh_token="))
        .expect("refresh_token clear cookie missing");
    assert!(refresh_clear.contains("Max-Age=0"));
}

#[sqlx::test]
async fn logout_token_not_reusable(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password123").await;
    let refresh = cookies["refresh_token"].clone();

    // Logout
    let resp = send(
        router.clone(),
        post_with_cookies("/api/auth/logout", &format!("refresh_token={refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // Try to use the same refresh token — should fail
    let resp = send(
        router,
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn logout_without_cookie_is_idempotent(pool: PgPool) {
    let resp = send(
        app(pool),
        Request::post("/api/auth/logout")
            .header("origin", FRONTEND_URL)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);
}

// =============================================================================
// Full flow
// =============================================================================

#[sqlx::test]
async fn full_flow_register_login_refresh_logout(pool: PgPool) {
    let router = app(pool);

    // 1. Register
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("flowuser", "flow@example.com", "password123"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let user_id = resp.json["user_id"].as_str().unwrap().to_string();

    // 2. Login
    let resp = send(
        router.clone(),
        post_json("/api/auth/login", &login_body("flowuser", "password123")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["user_id"], user_id);
    assert_eq!(resp.json["username"], "flowuser");
    let refresh_token = resp.cookies["refresh_token"].clone();

    // 3. Refresh — rotates token, returns same user
    let resp = send(
        router.clone(),
        post_with_cookies(
            "/api/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["user_id"], user_id);
    assert_eq!(resp.json["username"], "flowuser");
    let new_refresh = resp.cookies["refresh_token"].clone();
    assert_ne!(new_refresh, refresh_token);

    // 4. Logout with the new token
    let resp = send(
        router.clone(),
        post_with_cookies("/api/auth/logout", &format!("refresh_token={new_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // 5. The rotated-away token no longer works
    let resp = send(
        router.clone(),
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={new_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    // 6. The original token also doesn't work
    let resp = send(
        router,
        post_with_cookies(
            "/api/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}
