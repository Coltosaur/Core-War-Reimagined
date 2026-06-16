use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};
use std::time::Duration;

use crate::AppState;

const DB_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Cheap liveness probe. Always 200 if the process can serve requests.
/// Used by Cloudflare/UptimeRobot-style external probes — does no DB work,
/// so a hostile flood here can't amplify into database load.
pub async fn liveness() -> impl IntoResponse {
    let body: Json<Value> = Json(json!({ "status": "ok" }));
    (StatusCode::OK, body)
}

/// Deep readiness probe. Verifies the database is reachable. Used by the
/// container HEALTHCHECK so Docker can react to a downstream outage.
pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = check_db(&state).await;

    let (status_code, overall) = if db_ok {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded")
    };

    let body: Json<Value> = Json(json!({
        "status": overall,
        "database": if db_ok { "ok" } else { "fail" }
    }));

    (status_code, body)
}

async fn check_db(state: &AppState) -> bool {
    let query = sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db);
    matches!(
        tokio::time::timeout(DB_CHECK_TIMEOUT, query).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
