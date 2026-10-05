use crate::net::IpNet;
use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use redis::aio::ConnectionManager;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
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
    trusted_proxies: Arc<Vec<IpNet>>,
}

impl RateLimiter {
    pub fn new(
        conn: ConnectionManager,
        name: &str,
        max_requests: u32,
        window_secs: u64,
        trusted_proxies: Vec<IpNet>,
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

/// The client address to key the limit on, or `None` when the connection's
/// peer address is missing. A missing peer means the server was started
/// without `into_make_service_with_connect_info`; the caller must refuse the
/// request rather than guess, because any guess puts every client in one
/// shared bucket.
///
/// When the peer is a trusted proxy, `X-Forwarded-For` is read from the
/// right: each trusted proxy appends the address it received the request
/// from, so the rightmost entry that isn't a trusted proxy is the client.
/// Entries further left were written by the client and can't be trusted.
fn extract_ip(
    connect_info: Option<&ConnectInfo<SocketAddr>>,
    headers: &axum::http::HeaderMap,
    trusted_proxies: &[IpNet],
) -> Option<IpAddr> {
    let peer = connect_info?.0.ip();
    let trusted = |ip: IpAddr| trusted_proxies.iter().any(|net| net.contains(ip));

    if !trusted(peer) {
        if headers.contains_key("x-forwarded-for") && is_private(peer) {
            warn_untrusted_proxy_once(peer);
        }
        return Some(peer);
    }

    let mut client = peer;
    let hops = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .collect::<Vec<_>>();
    for hop in hops.into_iter().rev() {
        // Garbage from a trusted hop: stop at the last address we trust.
        let Ok(ip) = hop.trim().parse::<IpAddr>() else {
            break;
        };
        client = ip;
        if !trusted(ip) {
            break;
        }
    }
    Some(client)
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// A request forwarded by a proxy that isn't in `TRUSTED_PROXIES` gets keyed
/// on the proxy's address, so every client behind it shares one bucket. That
/// is a deploy misconfiguration, not a per-request problem: say so once.
fn warn_untrusted_proxy_once(peer: IpAddr) {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            %peer,
            "request carries X-Forwarded-For from an untrusted private peer; \
             rate limits will be shared by every client behind it. \
             Add the proxy to TRUSTED_PROXIES."
        );
    }
}

pub fn login_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpNet>) -> RateLimiter {
    RateLimiter::new(conn, "login", 5, 15 * 60, trusted_proxies)
}

pub fn register_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpNet>) -> RateLimiter {
    RateLimiter::new(conn, "register", 3, 60 * 60, trusted_proxies)
}

pub fn refresh_limiter(conn: ConnectionManager, trusted_proxies: Vec<IpNet>) -> RateLimiter {
    RateLimiter::new(conn, "refresh", 10, 15 * 60, trusted_proxies)
}

/// Change-password: 5 attempts per 15 min, same shape as login. The endpoint
/// verifies the current password so it's a plausible brute-force surface even
/// with a valid session cookie — cap it accordingly.
pub fn change_password_limiter(
    conn: ConnectionManager,
    trusted_proxies: Vec<IpNet>,
) -> RateLimiter {
    RateLimiter::new(conn, "change_password", 5, 15 * 60, trusted_proxies)
}

pub async fn rate_limit_middleware(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(ip) = extract_ip(
        connect_info.as_ref(),
        request.headers(),
        &limiter.trusted_proxies,
    ) else {
        tracing::error!(
            "rate limiter has no peer address; the server must be started \
             with into_make_service_with_connect_info"
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

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

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn peer(s: &str) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::new(ip(s), 12345))
    }

    fn xff(value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    fn nets(list: &[&str]) -> Vec<IpNet> {
        list.iter().map(|s| s.parse().unwrap()).collect()
    }

    #[test]
    fn missing_peer_address_yields_none_instead_of_a_shared_guess() {
        let headers = xff("203.0.113.50");
        assert_eq!(extract_ip(None, &headers, &nets(&["0.0.0.0/0"])), None);
    }

    #[test]
    fn direct_ip_used_when_no_trusted_proxies() {
        let headers = axum::http::HeaderMap::new();
        let got = extract_ip(Some(&peer("192.168.1.1")), &headers, &[]);
        assert_eq!(got, Some(ip("192.168.1.1")));
    }

    #[test]
    fn xff_ignored_when_peer_not_trusted() {
        let got = extract_ip(
            Some(&peer("192.168.1.1")),
            &xff("10.0.0.5"),
            &nets(&["172.16.0.1"]),
        );
        assert_eq!(got, Some(ip("192.168.1.1")));
    }

    #[test]
    fn xff_spoofing_blocked_without_trusted_proxy() {
        let got = extract_ip(Some(&peer("66.77.88.99")), &xff("1.1.1.1"), &[]);
        assert_eq!(got, Some(ip("66.77.88.99")));
    }

    #[test]
    fn client_address_taken_from_trusted_proxy() {
        let got = extract_ip(
            Some(&peer("172.19.0.4")),
            &xff("203.0.113.50"),
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("203.0.113.50")));
    }

    #[test]
    fn client_cannot_spoof_by_prepending_to_xff() {
        // The client sent `X-Forwarded-For: 1.2.3.4`; the proxy appended the
        // real address. Taking the leftmost entry would let every request
        // pick a fresh bucket.
        let got = extract_ip(
            Some(&peer("172.19.0.4")),
            &xff("1.2.3.4, 203.0.113.50"),
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("203.0.113.50")));
    }

    #[test]
    fn chained_trusted_proxies_are_skipped() {
        let got = extract_ip(
            Some(&peer("172.19.0.4")),
            &xff("1.2.3.4, 203.0.113.50, 172.20.0.9"),
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("203.0.113.50")));
    }

    #[test]
    fn repeated_xff_headers_are_read_in_order() {
        let mut headers = xff("1.2.3.4");
        headers.append("x-forwarded-for", "203.0.113.50".parse().unwrap());
        let got = extract_ip(
            Some(&peer("172.19.0.4")),
            &headers,
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("203.0.113.50")));
    }

    #[test]
    fn garbage_from_trusted_hop_falls_back_to_peer() {
        let got = extract_ip(
            Some(&peer("172.19.0.4")),
            &xff("not-an-ip"),
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("172.19.0.4")));
    }

    #[test]
    fn falls_back_to_peer_when_trusted_but_no_xff() {
        let headers = axum::http::HeaderMap::new();
        let got = extract_ip(Some(&peer("172.16.0.1")), &headers, &nets(&["172.16.0.1"]));
        assert_eq!(got, Some(ip("172.16.0.1")));
    }

    #[test]
    fn ipv4_mapped_peer_matches_ipv4_trusted_range() {
        let got = extract_ip(
            Some(&peer("::ffff:172.19.0.4")),
            &xff("203.0.113.50"),
            &nets(&["172.16.0.0/12"]),
        );
        assert_eq!(got, Some(ip("203.0.113.50")));
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
        let proxies = nets(&["10.0.0.1", "10.0.0.0/8"]);
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
