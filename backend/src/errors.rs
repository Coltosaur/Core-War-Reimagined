use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg),
            Self::Internal(msg) => {
                tracing::error!("internal error: {msg}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use serde_json::Value;

    async fn response_parts(error: AppError) -> (StatusCode, Value) {
        let response = error.into_response();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let (status, json) = response_parts(AppError::BadRequest("invalid input".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "invalid input");
    }

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let (status, json) =
            response_parts(AppError::Unauthorized("bad credentials".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "bad credentials");
    }

    #[tokio::test]
    async fn conflict_returns_409() {
        let (status, json) =
            response_parts(AppError::Conflict("username taken".into())).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"], "username taken");
    }

    #[tokio::test]
    async fn internal_returns_500_with_generic_message() {
        let (status, json) =
            response_parts(AppError::Internal("db connection failed".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"], "Internal server error");
    }

    #[tokio::test]
    async fn internal_does_not_leak_details() {
        let (_, json) =
            response_parts(AppError::Internal("secret db password exposed".into())).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(!error_msg.contains("secret"));
        assert!(!error_msg.contains("password"));
    }
}
