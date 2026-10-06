use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{auth, warriors};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use tower::ServiceExt;

mod common;

use common::{
    extract_cookies, get_with_cookies, post_json, post_json_with_cookies, test_state, FRONTEND_URL,
};

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/auth/login", post(auth::handlers::login))
        .route(
            "/api/warriors",
            get(warriors::handlers::list).post(warriors::handlers::create),
        )
        .route(
            "/api/warriors/:id",
            get(warriors::handlers::get)
                .put(warriors::handlers::update)
                .delete(warriors::handlers::delete),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

// --- Request builders ---

fn put_json_with_cookies(path: &str, body: &Value, cookies: &str) -> Request<Body> {
    Request::put(path)
        .header("content-type", "application/json")
        .header("origin", FRONTEND_URL)
        .header("cookie", cookies)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn delete_with_cookies(path: &str, cookies: &str) -> Request<Body> {
    Request::delete(path)
        .header("origin", FRONTEND_URL)
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

// --- Response helpers ---

struct TestResponse {
    status: StatusCode,
    json: Value,
}

async fn send(router: Router, req: Request<Body>) -> TestResponse {
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    TestResponse { status, json }
}

async fn send_with_cookies(
    router: Router,
    req: Request<Body>,
) -> (TestResponse, HashMap<String, String>) {
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let cookies = extract_cookies(resp.headers());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (TestResponse { status, json }, cookies)
}

// --- Auth helper ---

async fn register_and_login(router: &Router) -> String {
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({"username": "testuser", "email": "test@example.com", "password": "password1234"}),
        ),
    )
    .await;

    let (resp, cookies) = send_with_cookies(
        router.clone(),
        post_json(
            "/api/auth/login",
            &json!({"username_or_email": "testuser", "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    format!("access_token={}", cookies["access_token"])
}

async fn register_and_login_as(router: &Router, username: &str, email: &str) -> String {
    send(
        router.clone(),
        post_json(
            "/api/auth/register",
            &json!({"username": username, "email": email, "password": "password1234"}),
        ),
    )
    .await;

    let (resp, cookies) = send_with_cookies(
        router.clone(),
        post_json(
            "/api/auth/login",
            &json!({"username_or_email": username, "password": "password1234"}),
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    format!("access_token={}", cookies["access_token"])
}

fn warrior_body(name: &str, source: &str) -> Value {
    json!({"name": name, "source": source})
}

// =============================================================================
// Create tests
// =============================================================================

#[sqlx::test]
async fn create_warrior(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let resp = send(
        router,
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;

    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["name"], "Imp");
    assert_eq!(resp.json["source"], "MOV.I $0, $1");
    assert!(resp.json["id"].is_string());
    assert!(resp.json["user_id"].is_string());
    assert!(resp.json["created_at"].is_string());
    assert!(resp.json["updated_at"].is_string());
}

#[sqlx::test]
async fn create_warrior_unauthenticated(pool: PgPool) {
    let router = app(pool);
    let resp = send(
        router,
        post_json("/api/warriors", &warrior_body("Imp", "MOV.I $0, $1")),
    )
    .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn create_warrior_empty_name(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let resp = send(
        router,
        post_json_with_cookies("/api/warriors", &warrior_body("", "MOV.I $0, $1"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_warrior_name_too_long(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let long_name = "a".repeat(65);
    let resp = send(
        router,
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body(&long_name, "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_warrior_empty_source(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let resp = send(
        router,
        post_json_with_cookies("/api/warriors", &warrior_body("Imp", ""), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn create_warrior_name_trimmed(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let resp = send(
        router,
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("  Imp  ", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    assert_eq!(resp.json["name"], "Imp");
}

// =============================================================================
// Get tests
// =============================================================================

#[sqlx::test]
async fn get_warrior(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Imp");
    assert_eq!(resp.json["source"], "MOV.I $0, $1");
}

#[sqlx::test]
async fn get_warrior_not_found(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let fake_id = uuid::Uuid::new_v4();
    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{fake_id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn get_warrior_belongs_to_other_user(pool: PgPool) {
    let router = app(pool);
    let cookies_a = register_and_login_as(&router, "alice", "alice@example.com").await;
    let cookies_b = register_and_login_as(&router, "bob", "bob@example.com").await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Alice Imp", "MOV.I $0, $1"),
            &cookies_a,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies_b),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// List tests
// =============================================================================

#[sqlx::test]
async fn list_warriors_empty(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let resp = send(router, get_with_cookies("/api/warriors", &cookies)).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["warriors"].as_array().unwrap().len(), 0);
    assert_eq!(resp.json["total"], 0);
    assert_eq!(resp.json["page"], 1);
}

#[sqlx::test]
async fn list_warriors_returns_own_only(pool: PgPool) {
    let router = app(pool);
    let cookies_a = register_and_login_as(&router, "alice", "alice@example.com").await;
    let cookies_b = register_and_login_as(&router, "bob", "bob@example.com").await;

    send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Alice Imp", "MOV.I $0, $1"),
            &cookies_a,
        ),
    )
    .await;
    send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Bob Dwarf", "ADD.AB #4, $3"),
            &cookies_b,
        ),
    )
    .await;

    let resp = send(
        router.clone(),
        get_with_cookies("/api/warriors", &cookies_a),
    )
    .await;
    assert_eq!(resp.json["total"], 1);
    assert_eq!(resp.json["warriors"][0]["name"], "Alice Imp");

    let resp = send(router, get_with_cookies("/api/warriors", &cookies_b)).await;
    assert_eq!(resp.json["total"], 1);
    assert_eq!(resp.json["warriors"][0]["name"], "Bob Dwarf");
}

#[sqlx::test]
async fn list_warriors_pagination(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    for i in 0..5 {
        send(
            router.clone(),
            post_json_with_cookies(
                "/api/warriors",
                &warrior_body(&format!("Warrior {i}"), "MOV.I $0, $1"),
                &cookies,
            ),
        )
        .await;
    }

    let resp = send(
        router.clone(),
        get_with_cookies("/api/warriors?page=1&per_page=2", &cookies),
    )
    .await;
    assert_eq!(resp.json["warriors"].as_array().unwrap().len(), 2);
    assert_eq!(resp.json["total"], 5);
    assert_eq!(resp.json["page"], 1);
    assert_eq!(resp.json["per_page"], 2);

    let resp = send(
        router,
        get_with_cookies("/api/warriors?page=3&per_page=2", &cookies),
    )
    .await;
    assert_eq!(resp.json["warriors"].as_array().unwrap().len(), 1);
}

// =============================================================================
// Update tests
// =============================================================================

#[sqlx::test]
async fn update_warrior_name(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router,
        put_json_with_cookies(
            &format!("/api/warriors/{id}"),
            &json!({"name": "Super Imp"}),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Super Imp");
    assert_eq!(resp.json["source"], "MOV.I $0, $1");
}

#[sqlx::test]
async fn update_warrior_source(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router,
        put_json_with_cookies(
            &format!("/api/warriors/{id}"),
            &json!({"source": "ADD.AB #4, $3\nMOV.I $0, $1"}),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Imp");
    assert_eq!(resp.json["source"], "ADD.AB #4, $3\nMOV.I $0, $1");
}

#[sqlx::test]
async fn update_warrior_not_found(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let fake_id = uuid::Uuid::new_v4();
    let resp = send(
        router,
        put_json_with_cookies(
            &format!("/api/warriors/{fake_id}"),
            &json!({"name": "New Name"}),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn update_warrior_other_users(pool: PgPool) {
    let router = app(pool);
    let cookies_a = register_and_login_as(&router, "alice", "alice@example.com").await;
    let cookies_b = register_and_login_as(&router, "bob", "bob@example.com").await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Alice Imp", "MOV.I $0, $1"),
            &cookies_a,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router,
        put_json_with_cookies(
            &format!("/api/warriors/{id}"),
            &json!({"name": "Hacked!"}),
            &cookies_b,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Delete tests
// =============================================================================

#[sqlx::test]
async fn delete_warrior(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router.clone(),
        delete_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn delete_warrior_not_found(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    let fake_id = uuid::Uuid::new_v4();
    let resp = send(
        router,
        delete_with_cookies(&format!("/api/warriors/{fake_id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn delete_warrior_other_users(pool: PgPool) {
    let router = app(pool);
    let cookies_a = register_and_login_as(&router, "alice", "alice@example.com").await;
    let cookies_b = register_and_login_as(&router, "bob", "bob@example.com").await;

    let create_resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Alice Imp", "MOV.I $0, $1"),
            &cookies_a,
        ),
    )
    .await;
    let id = create_resp.json["id"].as_str().unwrap();

    let resp = send(
        router.clone(),
        delete_with_cookies(&format!("/api/warriors/{id}"), &cookies_b),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);

    // Verify it still exists for alice
    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies_a),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
}

// =============================================================================
// Full CRUD flow
// =============================================================================

#[sqlx::test]
async fn full_crud_flow(pool: PgPool) {
    let router = app(pool);
    let cookies = register_and_login(&router).await;

    // Create
    let resp = send(
        router.clone(),
        post_json_with_cookies(
            "/api/warriors",
            &warrior_body("Imp", "MOV.I $0, $1"),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::CREATED);
    let id = resp.json["id"].as_str().unwrap().to_string();

    // Read
    let resp = send(
        router.clone(),
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Imp");

    // List
    let resp = send(router.clone(), get_with_cookies("/api/warriors", &cookies)).await;
    assert_eq!(resp.json["total"], 1);

    // Update
    let resp = send(
        router.clone(),
        put_json_with_cookies(
            &format!("/api/warriors/{id}"),
            &json!({"name": "Super Imp", "source": ";name Super Imp\nMOV.I $0, $1"}),
            &cookies,
        ),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Super Imp");

    // Delete
    let resp = send(
        router.clone(),
        delete_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT);

    // Confirm deleted
    let resp = send(
        router,
        get_with_cookies(&format!("/api/warriors/{id}"), &cookies),
    )
    .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
}
