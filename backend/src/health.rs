use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};
use std::time::Duration;

use crate::AppState;

const DB_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
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
