use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{account, auth};
use http_body_util::BodyExt;
use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::net::SocketAddr;
use tower::ServiceExt;

mod common;

use common::{
    extract_cookies, get_with_cookies, post_json, post_json_with_cookies, test_state, FRONTEND_URL,
    JWT_SECRET,
};

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/auth/login", post(auth::handlers::login))
        .route("/api/auth/refresh", post(auth::handlers::refresh))
        .route("/api/auth/logout", post(auth::handlers::logout))
        .route(
            "/api/auth/change-password",
            post(auth::handlers::change_password),
        )
        // Mirror /api/account so tests can prove that the freshly-issued
        // access_token cookie the change-password handler returns actually
        // authenticates a subsequent protected request. Without this seam,
        // the cookie could be present-but-broken and no test would fire.
        .route("/api/account", get(account::handlers::me))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

/// Same as `app()`, but with login and change-password behind the
/// production failure-limit middleware. Callers pass limiters with a unique
/// Redis namespace and small limits, so assertions are tight and can't be
/// polluted by parallel tests or leftover Redis state.
fn app_with_failure_limits(
    pool: PgPool,
    login: auth::failure_limit::FailureLimiter,
    change_password: auth::failure_limit::FailureLimiter,
) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route(
            "/api/auth/login",
            post(auth::handlers::login).layer(middleware::from_fn_with_state(
                login,
                auth::failure_limit::failure_limit_middleware,
            )),
        )
        .route(
            "/api/auth/change-password",
            post(auth::handlers::change_password).layer(middleware::from_fn_with_state(
                change_password,
                auth::failure_limit::failure_limit_middleware,
            )),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

async fn failure_limiter(
    max_per_ip: u32,
    max_per_account: u32,
    source: auth::failure_limit::AccountSource,
) -> auth::failure_limit::FailureLimiter {
    let name = format!("test_{}", uuid::Uuid::new_v4());
    auth::failure_limit::FailureLimiter::new(
        test_redis_conn().await,
        &name,
        300,
        max_per_ip,
        max_per_account,
        source,
        vec![],
    )
}

/// Login limited at `max_per_ip` / `max_per_account`; change-password
/// effectively unlimited.
async fn login_limited_app(pool: PgPool, max_per_ip: u32, max_per_account: u32) -> Router {
    use auth::failure_limit::AccountSource;
    let login = failure_limiter(
        max_per_ip,
        max_per_account,
        AccountSource::LoginBody { db: pool.clone() },
    )
    .await;
    let change = failure_limiter(
        1000,
        1000,
        AccountSource::SessionUser {
            jwt_secret: JWT_SECRET.into(),
        },
    )
    .await;
    app_with_failure_limits(pool, login, change)
}

async fn test_redis_conn() -> ConnectionManager {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let client = redis::Client::open(url.as_str()).expect("open redis for rate-limit wiring test");
    ConnectionManager::new(client)
        .await
        .expect("connect redis for rate-limit wiring test")
}

/// A Redis connection routed through a local TCP relay, so a test can cut
/// it mid-run and watch the limiters lose Redis the way they would in an
/// outage: open connections drop and new ones are refused.
struct RedisRelay {
    tasks: std::sync::Arc<std::sync::Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl RedisRelay {
    async fn start() -> (Self, ConnectionManager) {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
        let target = url.trim_start_matches("redis://").to_string();
        let target = target
            .rsplit('@')
            .next()
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .to_string();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let relay = Self {
            tasks: Default::default(),
        };
        let tasks = relay.tasks.clone();
        let accept = tokio::spawn(async move {
            while let Ok((mut client, _)) = listener.accept().await {
                let target = target.clone();
                let pipe = tokio::spawn(async move {
                    let mut redis = tokio::net::TcpStream::connect(target).await.unwrap();
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut redis).await;
                });
                tasks.lock().unwrap().push(pipe.abort_handle());
            }
        });
        relay.tasks.lock().unwrap().push(accept.abort_handle());

        let client = redis::Client::open(format!("redis://127.0.0.1:{port}")).unwrap();
        let conn = ConnectionManager::new(client).await.unwrap();
        (relay, conn)
    }

    fn cut(&self) {
        for task in self.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
    }
}

// --- Request builders ---

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

async fn send(router: Router, req: Request<Body>) -> TestResponse {
    send_from(router, req, [127, 0, 0, 1]).await
}

/// `send` from a chosen client address.
async fn send_from(router: Router, mut req: Request<Body>, ip: [u8; 4]) -> TestResponse {
    // `oneshot` skips the server, so attach the peer address the real one
    // (`app::serve`) would. The rate limiters refuse requests without it.
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from((ip, 0))));
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
            &register_body("alice", "alice@example.com", "password1234"),
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
            &register_body("alice", "alice1@example.com", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("alice", "alice2@example.com", "password1234"),
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
            &register_body("user1", "same@example.com", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("user2", "same@example.com", "password1234"),
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
            &register_body("ab", "ab@example.com", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Username invalid chars
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("user!name", "un@example.com", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // Invalid email
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("validuser", "not-an-email", "password1234"),
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
    // username without password → 422 (missing required field). Email is
    // optional as of the make-email-optional change, so leaving it off
    // must NOT flip this to 422; the missing-password is the actual reason.
    let resp = send(
        app(pool),
        post_json("/api/auth/register", &json!({"username": "test"})),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn register_without_email_succeeds(pool: PgPool) {
    // Email is optional. Two flavors: omit the key entirely, and pass an
    // empty string (which the handler normalizes to None so it can't be
    // stored as a distinct "empty" address).
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({"username": "noemail1", "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["username"], "noemail1");

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &json!({"username": "noemail2", "email": "", "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["username"], "noemail2");
}

#[sqlx::test]
async fn register_multiple_without_email_no_conflict(pool: PgPool) {
    // Postgres UNIQUE treats NULLs as distinct — so two emailless users
    // must not trip the email uniqueness constraint. Regression guard: if
    // someone "fixes" the schema by adding a partial-unique-on-NULL or by
    // storing "" instead of NULL, this test flips to 409.
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({"username": "nullone", "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &json!({"username": "nulltwo", "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
}

#[sqlx::test]
async fn register_email_normalized(pool: PgPool) {
    let router = app(pool);

    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &register_body("alice", "  Alice@Example.COM  ", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);

    // Same email with different case should conflict
    let resp = send(
        router,
        post_json(
            "/api/auth/register",
            &register_body("bob", "alice@example.com", "password1234"),
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
            &register_body("  alice  ", "alice@example.com", "password1234"),
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
            &register_body("alice", "alice@example.com", "password1234"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password1234")),
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
            &register_body("alice", "alice@example.com", "password1234"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json(
            "/api/auth/login",
            &login_body("alice@example.com", "password1234"),
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
            &register_body("alice", "alice@example.com", "password1234"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json(
            "/api/auth/login",
            &login_body("ALICE@EXAMPLE.COM", "password1234"),
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
            &register_body("alice", "alice@example.com", "password1234"),
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
        post_json("/api/auth/login", &login_body("ghost", "password1234")),
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
            &register_body("alice", "alice@example.com", "password1234"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password1234")),
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
            &register_body("alice", "alice@example.com", "password1234"),
        ),
    )
    .await;

    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("alice", "password1234")),
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
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password1234").await;
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
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password1234").await;
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
            &register_body("alice", "alice@example.com", "password1234"),
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
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password1234").await;
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
    let cookies = register_and_login(&router, "alice", "alice@example.com", "password1234").await;
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
            &register_body("flowuser", "flow@example.com", "password1234"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let user_id = resp.json["user_id"].as_str().unwrap().to_string();

    // 2. Login
    let resp = send(
        router.clone(),
        post_json("/api/auth/login", &login_body("flowuser", "password1234")),
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

// =============================================================================
// Change-password tests
// =============================================================================

fn change_password_body(current: &str, new: &str) -> Value {
    json!({"current_password": current, "new_password": new})
}

#[sqlx::test]
async fn change_password_success_updates_hash_and_keeps_session(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "cpuser", "cp@example.com", "password1234").await;
    let access = cookies["access_token"].clone();

    // Change to a new password.
    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "brand-new-password"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // The response must issue a fresh access + refresh pair so the caller's
    // session survives the rotation.
    assert!(
        resp.cookies.contains_key("access_token"),
        "expected fresh access_token cookie"
    );
    assert!(
        resp.cookies.contains_key("refresh_token"),
        "expected fresh refresh_token cookie"
    );

    // Old password no longer works.
    let resp = send(
        router.clone(),
        post_json("/api/auth/login", &login_body("cpuser", "password1234")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    // New password works.
    let resp = send(
        router,
        post_json(
            "/api/auth/login",
            &login_body("cpuser", "brand-new-password"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
}

#[sqlx::test]
async fn change_password_wrong_current_rejected(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "cpwrong", "cpw@example.com", "password1234").await;
    let access = cookies["access_token"].clone();

    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("wrong-current-password", "brand-new-password"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(resp.json["error"], "Current password is incorrect");

    // Old password still works — nothing was changed.
    let resp = send(
        router,
        post_json("/api/auth/login", &login_body("cpwrong", "password1234")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
}

#[sqlx::test]
async fn change_password_rejects_weak_new_password(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "cpweak", "cpwk@example.com", "password1234").await;
    let access = cookies["access_token"].clone();

    // Too short.
    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "short"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    // All-numeric.
    let resp = send(
        router,
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "123456789012345"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn change_password_rejects_same_password(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "cpsame", "cps@example.com", "password1234").await;
    let access = cookies["access_token"].clone();

    let resp = send(
        router,
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "password1234"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let msg = resp.json["error"].as_str().unwrap();
    assert!(msg.to_lowercase().contains("different"));
}

#[sqlx::test]
async fn change_password_requires_auth(pool: PgPool) {
    let router = app(pool);

    let resp = send(
        router,
        post_json(
            "/api/auth/change-password",
            &change_password_body("anything", "brand-new-password"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn change_password_revokes_all_refresh_tokens(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router, "cprevoke", "cpr@example.com", "password1234").await;
    let access = cookies["access_token"].clone();
    let old_refresh = cookies["refresh_token"].clone();

    // Log in a *second* time to simulate a session on another device.
    let resp = send(
        router.clone(),
        post_json("/api/auth/login", &login_body("cprevoke", "password1234")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    let other_refresh = resp.cookies["refresh_token"].clone();

    // Change the password.
    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "brand-new-password"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // The refresh token from the initial login is invalidated.
    let resp = send(
        router.clone(),
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={old_refresh}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    // The refresh token from the "other device" is also invalidated — this is
    // the security-critical bit: changing your password kicks out every session.
    let resp = send(
        router,
        post_with_cookies(
            "/api/auth/refresh",
            &format!("refresh_token={other_refresh}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn change_password_new_access_cookie_authenticates_subsequent_request(pool: PgPool) {
    // After change-password, the fresh access_token cookie the response
    // hands back must actually authenticate a protected request. The
    // existing suite already asserts the cookie is *present* — this test
    // asserts it *works*. Prevents a regression where the handler builds a
    // cookie with the wrong claims / secret / expiry and everyone gets
    // silently logged out on their next authed action.
    let router = app(pool);
    let cookies = register_and_login(&router, "cpcook", "cpc@example.com", "password1234").await;
    let access = cookies["access_token"].clone();

    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "brand-new-password"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);
    let new_access = resp.cookies["access_token"].clone();
    // Deliberately not asserting new_access != access. HS256 JWTs are
    // deterministic in {claims × secret}, so a login + change-password
    // fired in the same wall-clock second produce byte-identical tokens.
    // That's not a bug — the point of this test is that the cookie the
    // response ships actually AUTHENTICATES, not that it's a new byte
    // sequence.
    let resp = send(
        router,
        get_with_cookies("/api/account", &format!("access_token={new_access}")),
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "freshly-issued access_token should authenticate /api/account: {:?}",
        resp.json
    );
    assert_eq!(resp.json["email"], "cpc@example.com");
}

#[sqlx::test]
async fn change_password_new_refresh_cookie_can_rotate(pool: PgPool) {
    // The refresh_token issued alongside the fresh access_token must be a
    // real, inserted-into-DB token — not a random string that happens to
    // parse. Round-trip it through /api/auth/refresh to prove the handler
    // wrote it to the refresh_tokens table with the correct hash. Guards
    // against a regression where the INSERT step is skipped or bound to the
    // wrong hash column.
    let router = app(pool);
    let cookies = register_and_login(&router, "cpref", "cpref@example.com", "password1234").await;
    let access = cookies["access_token"].clone();
    let old_refresh = cookies["refresh_token"].clone();

    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/auth/change-password",
            &change_password_body("password1234", "brand-new-password"),
            &format!("access_token={access}"),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);
    let new_refresh = resp.cookies["refresh_token"].clone();
    assert_ne!(new_refresh, old_refresh);

    // The fresh refresh cookie can be exchanged for another access+refresh
    // pair via the normal refresh flow. That proves the underlying token
    // row landed in `refresh_tokens` with the right expires_at and hash.
    let resp = send(
        router,
        post_with_cookies("/api/auth/refresh", &format!("refresh_token={new_refresh}")),
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "freshly-issued refresh_token should be accepted by /api/auth/refresh: {:?}",
        resp.json
    );
    assert!(resp.cookies.contains_key("refresh_token"));
    // The new refresh has been rotated by this call — it must not equal the
    // one we just used.
    assert_ne!(resp.cookies["refresh_token"], new_refresh);
}

#[sqlx::test]
async fn change_password_failures_lock_the_account(pool: PgPool) {
    // /api/auth/change-password checks the current password, so a stolen
    // session could be used to guess it. Wrong guesses count against the
    // signed-in account; once locked, even the right password is refused,
    // or an attacker could keep guessing and watch for the one success.
    use auth::failure_limit::AccountSource;
    let login = failure_limiter(1000, 1000, AccountSource::LoginBody { db: pool.clone() }).await;
    let change = failure_limiter(
        1000,
        2,
        AccountSource::SessionUser {
            jwt_secret: JWT_SECRET.into(),
        },
    )
    .await;
    let router = app_with_failure_limits(pool, login, change);

    let cookies = register_and_login(&router, "cplimit", "cpl@example.com", "password1234").await;
    let session = format!("access_token={}", cookies["access_token"]);
    let attempt = |current: &'static str, ip: [u8; 4]| {
        let router = router.clone();
        let session = session.clone();
        async move {
            send_from(
                router,
                post_json_with_cookies(
                    "/api/auth/change-password",
                    &change_password_body(current, "brand-new-password"),
                    &session,
                ),
                ip,
            )
            .await
        }
    };

    // Different addresses, so only the per-account counter can trip.
    assert_eq!(
        attempt("wrong-1", [10, 0, 0, 1]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        attempt("wrong-2", [10, 0, 0, 2]).await.status,
        StatusCode::UNAUTHORIZED
    );

    let resp = attempt("password1234", [10, 0, 0, 3]).await;
    assert_eq!(
        resp.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{:?}",
        resp.json
    );
    let err_msg = resp.json["error"].as_str().unwrap_or("");
    assert!(
        err_msg.to_lowercase().contains("too many"),
        "expected rate-limit copy; got {err_msg:?}"
    );
}

#[sqlx::test]
async fn successful_logins_do_not_count_toward_the_limit(pool: PgPool) {
    // The old limiter counted every login, so a handful of people signing
    // in from one network locked it out (#116).
    let router = login_limited_app(pool, 2, 2).await;
    register_and_login(&router, "regular", "regular@example.com", "password1234").await;

    for _ in 0..10 {
        let resp = send(
            router.clone(),
            post_json("/api/auth/login", &login_body("regular", "password1234")),
        )
        .await;
        assert_eq!(resp.status, StatusCode::OK, "{:?}", resp.json);
    }
}

#[sqlx::test]
async fn failed_logins_lock_the_account_from_every_address(pool: PgPool) {
    let router = login_limited_app(pool, 1000, 3).await;
    register_and_login(&router, "target", "target@example.com", "password1234").await;

    // A botnet or rotating VPN: every guess from a new address.
    for n in 1..=3 {
        let resp = send_from(
            router.clone(),
            post_json("/api/auth/login", &login_body("target", "guess")),
            [10, 0, 1, n],
        )
        .await;
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    // Locked, even with the right password and from a fresh address.
    let resp = send_from(
        router.clone(),
        post_json("/api/auth/login", &login_body("target", "password1234")),
        [10, 0, 1, 99],
    )
    .await;
    assert_eq!(resp.status, StatusCode::TOO_MANY_REQUESTS);

    // Logging in by email is the same account, so the same counter;
    // otherwise switching between username and email doubles the guesses.
    let resp = send_from(
        router.clone(),
        post_json(
            "/api/auth/login",
            &login_body("Target@Example.com", "password1234"),
        ),
        [10, 0, 1, 98],
    )
    .await;
    assert_eq!(resp.status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test]
async fn failed_logins_lock_the_address_across_accounts(pool: PgPool) {
    let router = login_limited_app(pool, 3, 1000).await;
    register_and_login(&router, "bystander", "by@example.com", "password1234").await;

    // One source guessing a different account each time.
    for name in ["acct1", "acct2", "acct3"] {
        let resp = send_from(
            router.clone(),
            post_json("/api/auth/login", &login_body(name, "guess")),
            [10, 0, 2, 1],
        )
        .await;
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    let locked = send_from(
        router.clone(),
        post_json("/api/auth/login", &login_body("bystander", "password1234")),
        [10, 0, 2, 1],
    )
    .await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);

    // Nobody else is affected.
    let other = send_from(
        router,
        post_json("/api/auth/login", &login_body("bystander", "password1234")),
        [10, 0, 2, 2],
    )
    .await;
    assert_eq!(other.status, StatusCode::OK);
}

#[sqlx::test]
async fn unknown_accounts_lock_exactly_like_real_ones(pool: PgPool) {
    // If only real accounts locked, the lockout would reveal which
    // usernames exist.
    let router = login_limited_app(pool, 1000, 2).await;
    register_and_login(&router, "realuser", "real@example.com", "password1234").await;

    for who in ["realuser", "no_such_user"] {
        let mut statuses = Vec::new();
        for n in 0..3 {
            let resp = send_from(
                router.clone(),
                post_json("/api/auth/login", &login_body(who, "guess")),
                [10, 0, 3, n],
            )
            .await;
            statuses.push(resp.status);
        }
        assert_eq!(
            statuses,
            [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::TOO_MANY_REQUESTS
            ],
            "{who}"
        );
    }
}

#[sqlx::test]
async fn successful_login_clears_the_account_failures(pool: PgPool) {
    // A player who mistypes twice and then gets it right starts fresh.
    let router = login_limited_app(pool, 1000, 3).await;
    register_and_login(&router, "typo", "typo@example.com", "password1234").await;
    let login = |password: &'static str| {
        send(
            router.clone(),
            post_json("/api/auth/login", &login_body("typo", password)),
        )
    };

    assert_eq!(login("oops1").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login("oops2").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login("password1234").await.status, StatusCode::OK);
    // Without the reset, the third of these would be the fourth failure.
    assert_eq!(login("oops3").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login("oops4").await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login("password1234").await.status, StatusCode::OK);
}

#[sqlx::test]
async fn failed_logins_still_lock_while_redis_is_down(pool: PgPool) {
    // During a Redis outage the limiters used to allow everything, which
    // switched off brute-force protection (#132). Now they count in memory
    // with the same limits.
    use auth::failure_limit::{AccountSource, FailureLimiter};
    let (relay, conn) = RedisRelay::start().await;
    let login = FailureLimiter::new(
        conn,
        &format!("test_{}", uuid::Uuid::new_v4()),
        300,
        3,
        2,
        AccountSource::LoginBody { db: pool.clone() },
        vec![],
    );
    let change = failure_limiter(
        1000,
        1000,
        AccountSource::SessionUser {
            jwt_secret: JWT_SECRET.into(),
        },
    )
    .await;
    let router = app_with_failure_limits(pool, login.clone(), change);
    register_and_login(&router, "outage", "outage@example.com", "password1234").await;

    relay.cut();

    // The account locks after two guesses from different addresses...
    for n in 1..=2 {
        let resp = send_from(
            router.clone(),
            post_json("/api/auth/login", &login_body("outage", "guess")),
            [10, 0, 2, n],
        )
        .await;
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }
    let resp = send_from(
        router.clone(),
        post_json("/api/auth/login", &login_body("outage", "password1234")),
        [10, 0, 2, 99],
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::TOO_MANY_REQUESTS,
        "{:?}",
        resp.json
    );
    assert!(login.is_degraded(), "the fallback should report itself");

    // ...and one address locks after three guesses across accounts.
    for name in ["ghost-a", "ghost-b", "ghost-c"] {
        let resp = send_from(
            router.clone(),
            post_json("/api/auth/login", &login_body(name, "guess")),
            [10, 0, 3, 1],
        )
        .await;
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }
    let resp = send_from(
        router.clone(),
        post_json("/api/auth/login", &login_body("ghost-d", "guess")),
        [10, 0, 3, 1],
    )
    .await;
    assert_eq!(resp.status, StatusCode::TOO_MANY_REQUESTS);
}
