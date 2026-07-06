use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use redis::aio::ConnectionManager;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

const RATE_LIMIT_SCRIPT: &str = r#"
local key = KEYS[1]
local max_requests = tonumber(ARGV[1])
local window_secs = tonumber(ARGV[2])
local now = tonumber(ARGV[3])
local request_id = ARGV[4]

local cutoff = now - window_secs

redis.call('ZREMRANGEBYSCORE', key, '-inf', cutoff)

local count = redis.call('ZCARD', key)
if count >= max_requests then
    local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
    if #oldest >= 2 then
        local retry_after = window_secs - (now - tonumber(oldest[2]))
        return retry_after
    end
    return window_secs
end

redis.call('ZADD', key, now, request_id)
redis.call('EXPIRE', key, window_secs + 1)

return -1
"#;

#[derive(Clone)]
pub struct RateLimiter {
    conn: ConnectionManager,
    key_prefix: String,
    max_requests: u32,
    window_secs: u64,
    trusted_proxies: Arc<Vec<IpAddr>>,
}

impl RateLimiter {
    pub fn new(
        conn: ConnectionManager,
        name: &str,
        max_requests: u32,
        window_secs: u64,
        trusted_proxies: Vec<IpAddr>,
    ) -> Self {
        Self {
            conn,
            key_prefix: format!("rate_limit:{name}"),
            max_requests,
            window_secs,
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    pub async fn check(&self, ip: IpAddr) -> Result<(), u64> {
        let key = format!("{}:{}", self.key_prefix, ip);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let request_id = format!("{now}:{}", uuid::Uuid::new_v4());
        let mut conn = self.conn.clone();

        let result: i64 = match redis::Script::new(RATE_LIMIT_SCRIPT)
            .key(&key)
            .arg(self.max_requests)
            .arg(self.window_secs)
            .arg(now)
            .arg(&request_id)
            .invoke_async(&mut conn)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Redis rate limit error: {e}");
                return Ok(());
            }
        };

        if result < 0 {
            Ok(())
        } else {
            Err(result as u64)
        }
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

pub fn login_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(conn, "login", 5, 15 * 60, trusted_proxies)
}

pub fn register_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(conn, "register", 3, 60 * 60, trusted_proxies)
}

pub fn refresh_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpAddr>) -> RateLimiter {
    RateLimiter::new(conn, "refresh", 10, 15 * 60, trusted_proxies)
}

/// Change-password: 5 attempts per 15 min, same shape as login. The endpoint
/// verifies the current password so it's a plausible brute-force surface even
/// with a valid session cookie — cap it accordingly.
pub fn change_password_limiter(
    conn: ConnectionManager,
    trusted_proxies: Vec<IpAddr>,
) -> RateLimiter {
    RateLimiter::new(conn, "change_password", 5, 15 * 60, trusted_proxies)
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

    match limiter.check(ip).await {
        Ok(()) => next.run(request).await,
        Err(retry_after) => {
            let secs = retry_after + 1;
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

    async fn test_conn() -> ConnectionManager {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
        let client = redis::Client::open(url.as_str()).unwrap();
        ConnectionManager::new(client).await.unwrap()
    }

    #[tokio::test]
    async fn login_limiter_config() {
        let l = login_limiter(test_conn().await, vec![]);
        assert_eq!(l.max_requests, 5);
        assert_eq!(l.window_secs, 15 * 60);
    }

    #[tokio::test]
    async fn register_limiter_config() {
        let l = register_limiter(test_conn().await, vec![]);
        assert_eq!(l.max_requests, 3);
        assert_eq!(l.window_secs, 60 * 60);
    }

    #[tokio::test]
    async fn refresh_limiter_config() {
        let l = refresh_limiter(test_conn().await, vec![]);
        assert_eq!(l.max_requests, 10);
        assert_eq!(l.window_secs, 15 * 60);
    }

    #[tokio::test]
    async fn change_password_limiter_config() {
        // The PR description advertised 5 attempts / 15min for the
        // change-password endpoint, matching the login limiter shape.
        // Pin those numbers so a silent bump in either the count or the
        // window fails this test — either would materially change the
        // brute-force surface characterization.
        let l = change_password_limiter(test_conn().await, vec![]);
        assert_eq!(l.max_requests, 5);
        assert_eq!(l.window_secs, 15 * 60);
    }

    #[tokio::test]
    async fn limiter_carries_trusted_proxies() {
        let proxies = vec![IpAddr::from([10, 0, 0, 1]), IpAddr::from([10, 0, 0, 2])];
        let l = login_limiter(test_conn().await, proxies.clone());
        assert_eq!(*l.trusted_proxies, proxies);
    }

    #[tokio::test]
    async fn check_allows_up_to_max() {
        let limiter = RateLimiter::new(test_conn().await, "test_allows", 3, 60, vec![]);
        let ip = IpAddr::from([1, 2, 3, 4]);

        let key = format!("{}:{}", limiter.key_prefix, ip);
        let mut c = limiter.conn.clone();
        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut c)
            .await
            .unwrap();

        assert!(limiter.check(ip).await.is_ok());
        assert!(limiter.check(ip).await.is_ok());
        assert!(limiter.check(ip).await.is_ok());
        assert!(limiter.check(ip).await.is_err());

        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut c)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn check_returns_retry_duration() {
        let limiter = RateLimiter::new(test_conn().await, "test_retry", 1, 300, vec![]);
        let ip = IpAddr::from([5, 6, 7, 8]);

        let key = format!("{}:{}", limiter.key_prefix, ip);
        let mut c = limiter.conn.clone();
        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut c)
            .await
            .unwrap();

        assert!(limiter.check(ip).await.is_ok());
        let retry = limiter.check(ip).await.unwrap_err();
        assert!(retry > 0 && retry <= 300);

        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut c)
            .await
            .unwrap();
    }
}
