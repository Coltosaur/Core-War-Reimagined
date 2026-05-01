use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde_json::json;
use uuid::Uuid;

use crate::auth::jwt::decode_access_token;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("access_token")
            .map(|c| c.value().to_string())
            .ok_or_else(|| unauthorized("Missing access token"))?;

        let claims = decode_access_token(&token, &state.config.jwt_secret)
            .map_err(|_| unauthorized("Invalid or expired token"))?;

        let user_id = claims
            .sub
            .parse::<Uuid>()
            .map_err(|_| unauthorized("Invalid token claims"))?;

        Ok(AuthUser {
            user_id,
            username: claims.username,
        })
    }
}

pub struct OptionalAuthUser(pub Option<AuthUser>);

#[async_trait]
impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuthUser(
            AuthUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}

fn unauthorized(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({ "error": msg }))).into_response()
}

pub async fn csrf_middleware(
    headers: HeaderMap,
    state: axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let method = request.method().clone();

    if matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(request).await;
    }

    let allowed_origin = &state.config.frontend_url;

    let origin_header = headers.get("origin").and_then(|v| v.to_str().ok());

    let referer_origin = headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .map(|r| {
            let parts: Vec<&str> = r.splitn(4, '/').collect();
            if parts.len() >= 3 {
                format!("{}//{}", parts[0], parts[2])
            } else {
                r.to_string()
            }
        });

    let origin = origin_header.map(|s| s.to_string()).or(referer_origin);

    match origin.as_deref() {
        Some(o) if o == allowed_origin => next.run(request).await,
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Origin not allowed" })),
        )
            .into_response(),
        None => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Missing origin" })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::middleware;
    use axum::routing::{get, post};
    use axum::Router;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::auth::jwt::encode_access_token;

    fn test_state() -> AppState {
        let pool = PgPool::connect_lazy("postgresql://localhost/fake").unwrap();
        AppState {
            db: pool,
            config: crate::AppConfig {
                frontend_url: "http://localhost:5173".into(),
                jwt_secret: b"this-is-a-test-secret-at-least-32-bytes!".to_vec(),
                trusted_proxies: vec![],
            },
        }
    }

    fn app_with_csrf() -> Router {
        let state = test_state();
        Router::new()
            .route("/test", post(|| async { "ok" }))
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                csrf_middleware,
            ))
            .with_state(state)
    }

    fn app_with_auth() -> Router {
        let state = test_state();
        Router::new()
            .route(
                "/protected",
                get(|user: AuthUser| async move { format!("hello {}", user.username) }),
            )
            .route(
                "/optional",
                get(|OptionalAuthUser(user): OptionalAuthUser| async move {
                    match user {
                        Some(u) => format!("hello {}", u.username),
                        None => "anonymous".into(),
                    }
                }),
            )
            .with_state(state)
    }

    async fn response_status_and_body(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap_or_else(|_| json!(body.to_vec()));
        (status, json)
    }

    // --- CSRF tests ---

    #[tokio::test]
    async fn csrf_allows_get_without_origin() {
        let app = app_with_csrf();
        let req = Request::get("/test").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_allows_post_with_correct_origin() {
        let app = app_with_csrf();
        let req = Request::post("/test")
            .header("origin", "http://localhost:5173")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_rejects_post_with_wrong_origin() {
        let app = app_with_csrf();
        let req = Request::post("/test")
            .header("origin", "http://evil.com")
            .body(Body::empty())
            .unwrap();
        let (status, json) = response_status_and_body(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["error"], "Origin not allowed");
    }

    #[tokio::test]
    async fn csrf_rejects_post_without_origin() {
        let app = app_with_csrf();
        let req = Request::post("/test").body(Body::empty()).unwrap();
        let (status, json) = response_status_and_body(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["error"], "Missing origin");
    }

    #[tokio::test]
    async fn csrf_allows_head_without_origin() {
        let app = app_with_csrf();
        let req = Request::head("/test").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_does_not_block_options_without_origin() {
        let app = app_with_csrf();
        let req = Request::options("/test").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // 405 comes from the router (no OPTIONS handler), not 403 from CSRF
        assert_ne!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn csrf_accepts_referer_as_fallback() {
        let app = app_with_csrf();
        let req = Request::post("/test")
            .header("referer", "http://localhost:5173/some/page")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_rejects_wrong_referer() {
        let app = app_with_csrf();
        let req = Request::post("/test")
            .header("referer", "http://evil.com/attack")
            .body(Body::empty())
            .unwrap();
        let (status, json) = response_status_and_body(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["error"], "Origin not allowed");
    }

    // --- AuthUser extractor tests ---

    #[tokio::test]
    async fn auth_rejects_missing_cookie() {
        let app = app_with_auth();
        let req = Request::get("/protected").body(Body::empty()).unwrap();
        let (status, json) = response_status_and_body(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "Missing access token");
    }

    #[tokio::test]
    async fn auth_rejects_invalid_token() {
        let app = app_with_auth();
        let req = Request::get("/protected")
            .header("cookie", "access_token=garbage")
            .body(Body::empty())
            .unwrap();
        let (status, json) = response_status_and_body(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "Invalid or expired token");
    }

    #[tokio::test]
    async fn auth_extracts_valid_user() {
        let state = test_state();
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "alice", &state.config.jwt_secret).unwrap();

        let app = app_with_auth();
        let req = Request::get("/protected")
            .header("cookie", format!("access_token={token}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "hello alice");
    }

    // --- OptionalAuthUser tests ---

    #[tokio::test]
    async fn optional_auth_returns_none_without_cookie() {
        let app = app_with_auth();
        let req = Request::get("/optional").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "anonymous");
    }

    #[tokio::test]
    async fn optional_auth_returns_user_with_valid_cookie() {
        let state = test_state();
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "bob", &state.config.jwt_secret).unwrap();

        let app = app_with_auth();
        let req = Request::get("/optional")
            .header("cookie", format!("access_token={token}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "hello bob");
    }

    #[tokio::test]
    async fn optional_auth_returns_none_with_invalid_cookie() {
        let app = app_with_auth();
        let req = Request::get("/optional")
            .header("cookie", "access_token=bad-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "anonymous");
    }
}
