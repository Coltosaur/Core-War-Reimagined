use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::time::Duration;

use crate::AppState;

const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Cheap liveness probe. Always 200 if the process can serve requests.
/// Used by Cloudflare/UptimeRobot-style external probes — does no DB work,
/// so a hostile flood here can't amplify into database load.
pub async fn liveness() -> impl IntoResponse {
    let body: Json<Value> = Json(json!({ "status": "ok" }));
    (StatusCode::OK, body)
}

/// Deep readiness probe. Verifies Postgres and Redis (matchmaking and the
/// rate limiters) are reachable, concurrently, each within its own timeout.
/// Used by the container HEALTHCHECK so Docker can react to a downstream
/// outage. The Redis connection comes from an `Extension` on this route, so
/// `AppState` and the many test states built from it don't need one.
pub async fn readiness(
    State(state): State<AppState>,
    Extension(redis): Extension<ConnectionManager>,
) -> impl IntoResponse {
    let (db_ok, redis_ok) = tokio::join!(check_db(&state.db), check_redis(redis));

    let (status_code, overall) = if db_ok && redis_ok {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded")
    };
    let label = |ok: bool| if ok { "ok" } else { "fail" };

    let body: Json<Value> = Json(json!({
        "status": overall,
        "database": label(db_ok),
        "redis": label(redis_ok),
    }));

    (status_code, body)
}

async fn check_db(db: &PgPool) -> bool {
    let query = sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(db);
    matches!(tokio::time::timeout(CHECK_TIMEOUT, query).await, Ok(Ok(_)))
}

/// `ConnectionManager` is a cheap clone of one shared multiplexed
/// connection, so this opens nothing new per request.
async fn check_redis(mut conn: ConnectionManager) -> bool {
    let cmd = redis::cmd("PING");
    let ping = cmd.query_async::<String>(&mut conn);
    matches!(tokio::time::timeout(CHECK_TIMEOUT, ping).await, Ok(Ok(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::limit_fallback::test_relay::RedisRelay;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn liveness_returns_200_with_status_ok() {
        let response = liveness().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn liveness_does_not_include_database_field() {
        // Critical: liveness must not reveal anything about downstream
        // state — otherwise it can't serve as a cheap probe.
        let response = liveness().await.into_response();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(
            body.get("database").is_none(),
            "liveness must not probe or expose database state"
        );
    }

    // Postgres is pointed at a dead port here: these tests are about the
    // Redis half (the DB half is covered in tests/health_integration.rs).
    // They need a real Redis, reached through the outage relay.
    async fn deep_with_redis(redis: ConnectionManager) -> (StatusCode, Value) {
        let state = AppState {
            db: PgPool::connect_lazy("postgresql://corewar:bogus@127.0.0.1:1/corewar").unwrap(),
            config: crate::AppConfig {
                frontend_url: "http://localhost:5173".into(),
                jwt_secret: b"this-is-a-test-secret-at-least-32-bytes!".to_vec(),
                trusted_proxies: vec![],
            },
        };
        let response = readiness(State(state), Extension(redis))
            .await
            .into_response();
        let status = response.status();
        let body =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        (status, body)
    }

    #[tokio::test]
    async fn readiness_reports_redis_ok_while_reachable() {
        let (_relay, conn) = RedisRelay::start().await;
        let (status, body) = deep_with_redis(conn).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "DB is down here");
        assert_eq!(body["redis"], "ok");
        assert_eq!(body["database"], "fail");
    }

    #[tokio::test]
    async fn readiness_returns_503_when_redis_unreachable() {
        let (relay, conn) = RedisRelay::start().await;
        relay.cut();
        let started = std::time::Instant::now();
        let (status, body) = deep_with_redis(conn).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "degraded");
        assert_eq!(body["redis"], "fail");
        assert!(
            started.elapsed() < CHECK_TIMEOUT + Duration::from_secs(1),
            "probes must run under the timeout, concurrently"
        );
    }
}
