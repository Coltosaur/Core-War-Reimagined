use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use core_war_backend::{health, AppConfig, AppState};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

const JWT_SECRET: &[u8] = b"integration-test-secret-that-is-at-least-32-bytes!!";

fn state(pool: PgPool) -> AppState {
    AppState {
        db: pool,
        config: AppConfig {
            frontend_url: "http://localhost:5173".into(),
            jwt_secret: JWT_SECRET.to_vec(),
            trusted_proxies: vec![],
        },
    }
}

fn app(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(health::liveness))
        .route("/health/deep", get(health::readiness))
        .with_state(state(pool))
}

#[sqlx::test]
async fn liveness_returns_ok_regardless_of_db(pool: PgPool) {
    let response = app(pool)
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(body["status"], "ok");
    assert!(
        body.get("database").is_none(),
        "liveness must not expose database state"
    );
}

#[sqlx::test]
async fn readiness_returns_ok_when_db_reachable(pool: PgPool) {
    let response = app(pool)
        .oneshot(Request::get("/health/deep").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "ok");
}

#[tokio::test]
async fn readiness_returns_503_when_db_unreachable() {
    // Lazy pool pointed at a port nothing's listening on. The 2s timeout in
    // the readiness handler caps the test runtime.
    let pool = PgPool::connect_lazy("postgresql://corewar:bogus@127.0.0.1:1/corewar").unwrap();

    let response = app(pool)
        .oneshot(Request::get("/health/deep").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"], "fail");
}

#[tokio::test]
async fn liveness_responds_when_db_unreachable() {
    // Key invariant: even with a dead DB, /health returns 200 instantly —
    // that's the whole point of splitting it from readiness.
    let pool = PgPool::connect_lazy("postgresql://corewar:bogus@127.0.0.1:1/corewar").unwrap();

    let response = app(pool)
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
