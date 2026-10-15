use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Extension, Router};
use core_war_backend::health;
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

mod common;

use common::test_state;

async fn app(pool: PgPool) -> Router {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let redis = redis::aio::ConnectionManager::new(redis::Client::open(url).unwrap())
        .await
        .unwrap();
    Router::new()
        .route("/health", get(health::liveness))
        .route(
            "/health/deep",
            get(health::readiness).layer(Extension(redis)),
        )
        .with_state(test_state(pool))
}

#[sqlx::test]
async fn liveness_returns_ok_regardless_of_db(pool: PgPool) {
    let response = app(pool)
        .await
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
        .await
        .oneshot(Request::get("/health/deep").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["redis"], "ok");
}

#[tokio::test]
async fn readiness_returns_503_when_db_unreachable() {
    // Lazy pool pointed at a port nothing's listening on. The 2s timeout in
    // the readiness handler caps the test runtime.
    let pool = PgPool::connect_lazy("postgresql://corewar:bogus@127.0.0.1:1/corewar").unwrap();

    let response = app(pool)
        .await
        .oneshot(Request::get("/health/deep").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"], "fail");
    assert_eq!(body["redis"], "ok");
}

#[tokio::test]
async fn liveness_responds_when_db_unreachable() {
    // Key invariant: even with a dead DB, /health returns 200 instantly —
    // that's the whole point of splitting it from readiness.
    let pool = PgPool::connect_lazy("postgresql://corewar:bogus@127.0.0.1:1/corewar").unwrap();

    let response = app(pool)
        .await
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
