use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{account, auth, AppConfig, AppState};
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
        .route("/api/account", get(account::handlers::me))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

fn post_json(path: &str, body: &Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("origin", FRONTEND_URL)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get_with_cookies(path: &str, cookies: &str) -> Request<Body> {
    Request::get(path)
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

fn get_no_auth(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

struct TestResponse {
    status: StatusCode,
    json: Value,
    cookies: HashMap<String, String>,
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
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    TestResponse {
        status,
        json,
        cookies,
    }
}

async fn register_user(router: &Router, username: &str, email: &str) -> HashMap<String, String> {
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({
                "username": username,
                "email": email,
                "password": "password1234",
            }),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    resp.cookies
}

#[sqlx::test]
async fn account_returns_email_for_authed_user(pool: PgPool) {
    let router = app(pool);
    let cookies = register_user(&router, "acctuser", "acct@example.com").await;
    let access = cookies["access_token"].clone();

    let resp = send(
        router,
        get_with_cookies("/api/account", &format!("access_token={access}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["email"], "acct@example.com");
}

#[sqlx::test]
async fn account_returns_null_email_when_none_registered(pool: PgPool) {
    // Users can register without an email (see auth_integration:
    // register_without_email_succeeds). /api/account must round-trip that
    // as JSON null, not as "" or a missing key — the frontend uses
    // `email === null` as the "not set" signal.
    let router = app(pool);
    let resp = send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({
                "username": "acctnoemail",
                "password": "password1234",
            }),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let access = resp.cookies["access_token"].clone();

    let resp = send(
        router,
        get_with_cookies("/api/account", &format!("access_token={access}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(
        resp.json["email"].is_null(),
        "expected null, got {:?}",
        resp.json["email"]
    );
}

#[sqlx::test]
async fn account_requires_authentication(pool: PgPool) {
    let router = app(pool);
    let resp = send(router, get_no_auth("/api/account")).await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn account_normalizes_email_to_lowercase(pool: PgPool) {
    // Registration lowercases the email — /api/account should reflect that
    // stored form so the settings page can display it verbatim.
    let router = app(pool);
    let cookies = register_user(&router, "acctcase", "AcCt+Mixed@ExAmPlE.COM").await;
    let access = cookies["access_token"].clone();

    let resp = send(
        router,
        get_with_cookies("/api/account", &format!("access_token={access}")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["email"], "acct+mixed@example.com");
}
