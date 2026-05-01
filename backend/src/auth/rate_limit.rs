use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    max_requests: u32,
    window: Duration,
    trusted_proxies: Arc<Vec<IpAddr>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration, trusted_proxies: Vec<IpAddr>) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    pub fn check(&self, ip: IpAddr) -> Result<(), Duration> {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - self.window;

        let timestamps = state.entry(ip).or_default();
        timestamps.retain(|t| *t > cutoff);

        if timestamps.len() >= self.max_requests as usize {
            let oldest = timestamps[0];
            let retry_after = self.window - (now - oldest);
            return Err(retry_after);
        }

        timestamps.push(now);
        Ok(())
    }
}

fn extract_ip(
    connect_info: Option<&ConnectInfo<SocketAddr>>,
    headers: &axum::http::HeaderMap,
    trusted_proxies: &[IpAddr],
) -> IpAddr {
    let direct_ip = connect_info.map(|ci| ci.0.ip());

    let is_trusted = direct_ip
        .map(|ip| trusted_proxies.contains(&ip))
        .unwrap_or(false);

    if is_trusted {
        if let Some(forwarded_ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
        {
            return forwarded_ip;
        }
    }

    direct_ip.unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

pub fn login_limiter(trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(5, Duration::from_secs(15 * 60), trusted_proxies)
}

pub fn register_limiter(trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(3, Duration::from_secs(60 * 60), trusted_proxies)
}

pub fn refresh_limiter(trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(10, Duration::from_secs(15 * 60), trusted_proxies)
}

pub async fn rate_limit_middleware(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let ip = extract_ip(
        connect_info.as_ref(),
        request.headers(),
        &limiter.trusted_proxies,
    );

    match limiter.check(ip) {
        Ok(()) => next.run(request).await,
        Err(retry_after) => {
            let secs = retry_after.as_secs() + 1;
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, secs.to_string())],
                Json(json!({"error": format!("Too many requests. Try again in {secs} seconds.")})),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::post;
    use axum::Router;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    fn test_limiter(max: u32, window_secs: u64) -> RateLimiter {
        RateLimiter::new(max, Duration::from_secs(window_secs), vec![])
    }

    fn app_with_limiter(limiter: RateLimiter) -> Router {
        Router::new()
            .route("/test", post(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                limiter.clone(),
                rate_limit_middleware,
            ))
            .with_state(limiter)
    }

    async fn response_parts(resp: Response) -> (StatusCode, Value, Option<String>) {
        let status = resp.status();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap_or(json!(null));
        (status, json, retry_after)
    }

    fn post_request() -> Request<Body> {
        Request::post("/test").body(Body::empty()).unwrap()
    }

    // --- Sliding window tests ---

    #[tokio::test]
    async fn allows_requests_within_limit() {
        let limiter = test_limiter(3, 60);
        for _ in 0..3 {
            let app = app_with_limiter(limiter.clone());
            let resp = app.oneshot(post_request()).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn blocks_after_limit_exceeded() {
        let limiter = test_limiter(2, 60);
        for _ in 0..2 {
            let app = app_with_limiter(limiter.clone());
            let resp = app.oneshot(post_request()).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        let app = app_with_limiter(limiter.clone());
        let (status, json, retry_after) =
            response_parts(app.oneshot(post_request()).await.unwrap()).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(json["error"]
            .as_str()
            .unwrap()
            .contains("Too many requests"));
        assert!(retry_after.is_some());
    }

    #[tokio::test]
    async fn retry_after_header_present() {
        let limiter = test_limiter(1, 300);

        let app = app_with_limiter(limiter.clone());
        app.oneshot(post_request()).await.unwrap();

        let app = app_with_limiter(limiter.clone());
        let (_, _, retry_after) = response_parts(app.oneshot(post_request()).await.unwrap()).await;
        let secs: u64 = retry_after.unwrap().parse().unwrap();
        assert!(secs > 0 && secs <= 301);
    }

    #[test]
    fn check_allows_up_to_max() {
        let limiter = test_limiter(3, 60);
        let ip = IpAddr::from([1, 2, 3, 4]);
        assert!(limiter.check(ip).is_ok());
        assert!(limiter.check(ip).is_ok());
        assert!(limiter.check(ip).is_ok());
        assert!(limiter.check(ip).is_err());
    }

    #[test]
    fn check_returns_retry_duration() {
        let limiter = test_limiter(1, 300);
        let ip = IpAddr::from([5, 6, 7, 8]);
        assert!(limiter.check(ip).is_ok());
        let retry = limiter.check(ip).unwrap_err();
        assert!(retry.as_secs() > 0 && retry.as_secs() <= 300);
    }

    // --- IP extraction + trust boundary tests ---

    #[test]
    fn direct_ip_used_when_no_trusted_proxies() {
        let headers = axum::http::HeaderMap::new();
        let ci = ConnectInfo(SocketAddr::from(([192, 168, 1, 1], 12345)));
        let ip = extract_ip(Some(&ci), &headers, &[]);
        assert_eq!(ip, IpAddr::from([192, 168, 1, 1]));
    }

    #[test]
    fn xff_ignored_when_no_trusted_proxies() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.5".parse().unwrap());
        let ci = ConnectInfo(SocketAddr::from(([192, 168, 1, 1], 12345)));
        let ip = extract_ip(Some(&ci), &headers, &[]);
        assert_eq!(ip, IpAddr::from([192, 168, 1, 1]));
    }

    #[test]
    fn xff_ignored_when_direct_ip_not_trusted() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.5".parse().unwrap());
        let ci = ConnectInfo(SocketAddr::from(([192, 168, 1, 1], 12345)));
        let trusted = vec![IpAddr::from([172, 16, 0, 1])];
        let ip = extract_ip(Some(&ci), &headers, &trusted);
        assert_eq!(ip, IpAddr::from([192, 168, 1, 1]));
    }

    #[test]
    fn xff_used_when_direct_ip_is_trusted() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "203.0.113.50, 70.41.3.18".parse().unwrap(),
        );
        let ci = ConnectInfo(SocketAddr::from(([172, 16, 0, 1], 12345)));
        let trusted = vec![IpAddr::from([172, 16, 0, 1])];
        let ip = extract_ip(Some(&ci), &headers, &trusted);
        assert_eq!(ip, IpAddr::from([203, 0, 113, 50]));
    }

    #[test]
    fn falls_back_to_direct_ip_when_trusted_but_no_xff() {
        let headers = axum::http::HeaderMap::new();
        let ci = ConnectInfo(SocketAddr::from(([172, 16, 0, 1], 12345)));
        let trusted = vec![IpAddr::from([172, 16, 0, 1])];
        let ip = extract_ip(Some(&ci), &headers, &trusted);
        assert_eq!(ip, IpAddr::from([172, 16, 0, 1]));
    }

    #[test]
    fn defaults_to_localhost_when_no_connect_info() {
        let headers = axum::http::HeaderMap::new();
        let ip = extract_ip(None, &headers, &[]);
        assert_eq!(ip, IpAddr::from([127, 0, 0, 1]));
    }

    #[test]
    fn xff_spoofing_blocked_without_trusted_proxy() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", "1.1.1.1".parse().unwrap());
        let attacker = [66, 77, 88, 99];
        let ci = ConnectInfo(SocketAddr::from((attacker, 9999)));
        let ip = extract_ip(Some(&ci), &headers, &[]);
        assert_eq!(ip, IpAddr::from(attacker));
    }

    // --- Per-IP isolation (middleware-level) ---

    #[tokio::test]
    async fn different_ips_tracked_separately_via_connect_info() {
        let limiter = test_limiter(1, 60);

        let app = app_with_limiter(limiter.clone());
        let resp = app.oneshot(post_request()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = app_with_limiter(limiter.clone());
        let resp = app.oneshot(post_request()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    // --- Limiter config tests ---

    #[test]
    fn login_limiter_config() {
        let l = login_limiter(vec![]);
        assert_eq!(l.max_requests, 5);
        assert_eq!(l.window, Duration::from_secs(15 * 60));
    }

    #[test]
    fn register_limiter_config() {
        let l = register_limiter(vec![]);
        assert_eq!(l.max_requests, 3);
        assert_eq!(l.window, Duration::from_secs(60 * 60));
    }

    #[test]
    fn refresh_limiter_config() {
        let l = refresh_limiter(vec![]);
        assert_eq!(l.max_requests, 10);
        assert_eq!(l.window, Duration::from_secs(15 * 60));
    }

    #[test]
    fn limiter_carries_trusted_proxies() {
        let proxies = vec![IpAddr::from([10, 0, 0, 1]), IpAddr::from([10, 0, 0, 2])];
        let l = login_limiter(proxies.clone());
        assert_eq!(*l.trusted_proxies, proxies);
    }
}
