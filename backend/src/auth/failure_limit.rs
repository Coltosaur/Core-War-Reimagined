//! Failed-attempt limits for endpoints that check a password (#116).
//!
//! The per-request limiter in `rate_limit` counts every request, which is
//! right for register and refresh but wrong here: it counted successful
//! logins, so five people logging in from one network locked it out. This
//! limiter counts only attempts the handler rejected with 401, against two
//! counters:
//!
//! - **Per IP**, so one source can't guess across many accounts.
//! - **Per account**, with a lower limit, so guesses spread across many IPs
//!   (a botnet, VPN rotation) can't target one account.
//!
//! A locked request is refused before the handler runs, even if its password
//! is right; otherwise an attacker could keep guessing and simply watch for
//! the one success. A successful attempt clears that account's failures but
//! not the IP's, so logging into an account you own can't reset the IP
//! counter between guesses at someone else's.
//!
//! The per-account counter alone would let anyone lock a specific player out
//! by failing logins as them, which is why the IP counter has to exist too —
//! and why the account limit is a short sliding window rather than a lock
//! that needs an admin to clear.

use crate::app::MAX_REQUEST_BODY_BYTES;
use crate::auth::jwt::decode_access_token;
use crate::auth::rate_limit::{extract_ip, missing_peer_address, too_many_requests};
use crate::net::IpNet;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use redis::aio::ConnectionManager;
use serde::Deserialize;
use sqlx::PgPool;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// Refuses when any key has `max` or more failures in the window. Returns
/// the seconds until the oldest blocking failure ages out, or -1 if allowed.
/// KEYS: the counters; ARGV: window, now, then one max per key.
const CHECK_SCRIPT: &str = r#"
local window = tonumber(ARGV[1])
local now = tonumber(ARGV[2])
local retry = -1
for i, key in ipairs(KEYS) do
    redis.call('ZREMRANGEBYSCORE', key, '-inf', now - window)
    if redis.call('ZCARD', key) >= tonumber(ARGV[2 + i]) then
        local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
        local wait = window - (now - tonumber(oldest[2]))
        if wait > retry then
            retry = wait
        end
    end
end
return retry
"#;

/// Adds one failure to every key. ARGV: window, now, unique member.
const RECORD_SCRIPT: &str = r#"
local window = tonumber(ARGV[1])
for _, key in ipairs(KEYS) do
    redis.call('ZADD', key, ARGV[2], ARGV[3])
    redis.call('EXPIRE', key, window + 1)
end
return 1
"#;

/// Where the account an attempt targets comes from.
#[derive(Clone)]
pub enum AccountSource {
    /// The `username_or_email` field of a login request's JSON body,
    /// resolved to the user it names so that logging in by username and by
    /// email share one counter.
    LoginBody { db: PgPool },
    /// The signed-in user, from the access-token cookie.
    SessionUser { jwt_secret: Arc<[u8]> },
}

#[derive(Clone)]
pub struct FailureLimiter {
    conn: ConnectionManager,
    key_prefix: String,
    window_secs: u64,
    max_per_ip: u32,
    max_per_account: u32,
    account_source: AccountSource,
    trusted_proxies: Arc<Vec<IpNet>>,
}

impl FailureLimiter {
    pub fn new(
        conn: ConnectionManager,
        name: &str,
        window_secs: u64,
        max_per_ip: u32,
        max_per_account: u32,
        account_source: AccountSource,
        trusted_proxies: Vec<IpNet>,
    ) -> Self {
        Self {
            conn,
            // Under `rate_limit:` so the deploy README's `--scan` check
            // finds these alongside the per-request limiters.
            key_prefix: format!("rate_limit:{name}_failures"),
            window_secs,
            max_per_ip,
            max_per_account,
            account_source,
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    fn ip_key(&self, ip: IpAddr) -> String {
        format!("{}:ip:{ip}", self.key_prefix)
    }

    fn account_key(&self, account: &str) -> String {
        format!("{}:account:{account}", self.key_prefix)
    }

    fn keys(&self, ip: IpAddr, account: Option<&str>) -> Vec<(String, u32)> {
        let mut keys = vec![(self.ip_key(ip), self.max_per_ip)];
        if let Some(account) = account {
            keys.push((self.account_key(account), self.max_per_account));
        }
        keys
    }

    /// `Err(seconds)` when the IP or the account is locked out.
    pub async fn check(&self, ip: IpAddr, account: Option<&str>) -> Result<(), u64> {
        let keys = self.keys(ip, account);
        let check = redis::Script::new(CHECK_SCRIPT);
        let mut script = check.prepare_invoke();
        for (key, _) in &keys {
            script.key(key);
        }
        script.arg(self.window_secs).arg(now_secs());
        for (_, max) in &keys {
            script.arg(*max);
        }
        let mut conn = self.conn.clone();
        match script.invoke_async::<i64>(&mut conn).await {
            Ok(retry) if retry >= 0 => Err(retry as u64),
            Ok(_) => Ok(()),
            Err(e) => {
                // Matches the per-request limiter. Whether an outage
                // should block logins instead is an open question on #116.
                tracing::error!("Redis failure-limit check error: {e}");
                Ok(())
            }
        }
    }

    pub async fn record_failure(&self, ip: IpAddr, account: Option<&str>) {
        let record = redis::Script::new(RECORD_SCRIPT);
        let mut script = record.prepare_invoke();
        for (key, _) in self.keys(ip, account) {
            script.key(key);
        }
        script
            .arg(self.window_secs)
            .arg(now_secs())
            .arg(uuid::Uuid::new_v4().to_string());
        let mut conn = self.conn.clone();
        if let Err(e) = script.invoke_async::<i64>(&mut conn).await {
            tracing::error!("Redis failure-limit record error: {e}");
        }
    }

    pub async fn clear_account(&self, account: &str) {
        let mut conn = self.conn.clone();
        let result: Result<(), _> = redis::cmd("DEL")
            .arg(self.account_key(account))
            .query_async(&mut conn)
            .await;
        if let Err(e) = result {
            tracing::error!("Redis failure-limit clear error: {e}");
        }
    }

    /// Reads the targeted account, returning the request rebuilt if its
    /// body had to be consumed. `Err` is a response to send as-is.
    async fn account_of(&self, request: Request) -> Result<(Option<String>, Request), Response> {
        match &self.account_source {
            AccountSource::LoginBody { db } => {
                let (parts, body) = request.into_parts();
                let bytes = axum::body::to_bytes(body, MAX_REQUEST_BODY_BYTES)
                    .await
                    .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE.into_response())?;
                // A body that doesn't parse has no account; the handler
                // rejects it, and only the IP counter applies.
                let account = match serde_json::from_slice::<LoginTarget>(&bytes) {
                    Ok(target) => login_account(db, &target.username_or_email).await,
                    Err(_) => None,
                };
                Ok((account, Request::from_parts(parts, Body::from(bytes))))
            }
            AccountSource::SessionUser { jwt_secret } => {
                let account = CookieJar::from_headers(request.headers())
                    .get("access_token")
                    .and_then(|c| decode_access_token(c.value(), jwt_secret).ok())
                    .map(|claims| format!("user:{}", claims.sub));
                Ok((account, request))
            }
        }
    }
}

#[derive(Deserialize)]
struct LoginTarget {
    username_or_email: String,
}

/// The counter a login attempt is charged to. An existing account is keyed
/// by its id, found with the same lookup as the login handler, so guesses by
/// username and by email share one counter. Anything else is keyed by the
/// normalized input, so unknown names lock exactly like real ones and the
/// lockout can't be used to discover which usernames exist.
///
/// One weak signal remains: after ten failures as a username, a locked
/// response to its email shows the two belong together. It costs ten
/// guesses per probe and is bounded by the IP counter.
async fn login_account(db: &PgPool, raw: &str) -> Option<String> {
    let input = raw.trim();
    let user_id: Option<uuid::Uuid> =
        match sqlx::query_scalar("SELECT id FROM users WHERE username = $1 OR email = $2")
            .bind(input)
            .bind(input.to_lowercase())
            .fetch_optional(db)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("failure limiter account lookup: {e}");
                None
            }
        };
    match user_id {
        Some(id) => Some(format!("user:{id}")),
        None => normalize_account(input).map(|name| format!("name:{name}")),
    }
}

/// Case-insensitive so `Ghost`, `ghost` and ` ghost ` share one counter
/// when no account matches.
fn normalize_account(raw: &str) -> Option<String> {
    let account = raw.trim().to_lowercase();
    (!account.is_empty()).then_some(account)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub async fn failure_limit_middleware(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    State(limiter): State<FailureLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let Some(ip) = extract_ip(
        connect_info.as_ref(),
        request.headers(),
        &limiter.trusted_proxies,
    ) else {
        return missing_peer_address();
    };

    let (account, request) = match limiter.account_of(request).await {
        Ok(found) => found,
        Err(response) => return response,
    };

    if let Err(retry_after) = limiter.check(ip, account.as_deref()).await {
        return too_many_requests(retry_after);
    }

    let response = next.run(request).await;

    if response.status() == StatusCode::UNAUTHORIZED {
        limiter.record_failure(ip, account.as_deref()).await;
    } else if response.status().is_success() {
        if let Some(account) = &account {
            limiter.clear_account(account).await;
        }
    }
    response
}

const WINDOW_SECS: u64 = 15 * 60;

/// Login: 20 failures per IP and 10 per account in 15 minutes. Successful
/// logins don't count.
pub fn login_failure_limiter(
    conn: ConnectionManager,
    trusted_proxies: Vec<IpNet>,
    db: PgPool,
) -> FailureLimiter {
    FailureLimiter::new(
        conn,
        "login",
        WINDOW_SECS,
        20,
        10,
        AccountSource::LoginBody { db },
        trusted_proxies,
    )
}

/// Change-password checks the current password, so a stolen session could
/// be used to guess it. Same limits as login, keyed on the signed-in user.
pub fn change_password_failure_limiter(
    conn: ConnectionManager,
    trusted_proxies: Vec<IpNet>,
    jwt_secret: &[u8],
) -> FailureLimiter {
    FailureLimiter::new(
        conn,
        "change_password",
        WINDOW_SECS,
        20,
        10,
        AccountSource::SessionUser {
            jwt_secret: Arc::from(jwt_secret),
        },
        trusted_proxies,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn conn() -> ConnectionManager {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
        ConnectionManager::new(redis::Client::open(url.as_str()).unwrap())
            .await
            .unwrap()
    }

    async fn limiter(max_per_ip: u32, max_per_account: u32) -> FailureLimiter {
        // A fresh namespace per test: no cleanup races, no leftover state.
        let name = format!("test_{}", uuid::Uuid::new_v4());
        FailureLimiter::new(
            conn().await,
            &name,
            300,
            max_per_ip,
            max_per_account,
            AccountSource::SessionUser {
                jwt_secret: Arc::from(&b"unused"[..]),
            },
            vec![],
        )
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn production_limits() {
        // Pinned: changing any of these changes the brute-force surface.
        let db = PgPool::connect_lazy("postgresql://localhost/unused").unwrap();
        let login = login_failure_limiter(conn().await, vec![], db);
        assert_eq!(
            (login.window_secs, login.max_per_ip, login.max_per_account),
            (15 * 60, 20, 10)
        );
        assert!(matches!(
            login.account_source,
            AccountSource::LoginBody { .. }
        ));
        assert_eq!(login.key_prefix, "rate_limit:login_failures");

        let change = change_password_failure_limiter(conn().await, vec![], b"secret");
        assert_eq!(
            (
                change.window_secs,
                change.max_per_ip,
                change.max_per_account
            ),
            (15 * 60, 20, 10)
        );
        assert!(matches!(
            change.account_source,
            AccountSource::SessionUser { .. }
        ));
    }

    #[test]
    fn unknown_names_are_normalized() {
        assert_eq!(normalize_account("  Ghost "), Some("ghost".into()));
        assert_eq!(normalize_account("A@B.com"), Some("a@b.com".into()));
        assert_eq!(normalize_account("   "), None);
    }

    #[tokio::test]
    async fn locks_an_ip_after_max_failures() {
        let l = limiter(3, 100).await;
        for _ in 0..3 {
            assert!(l.check(ip("1.1.1.1"), None).await.is_ok());
            l.record_failure(ip("1.1.1.1"), None).await;
        }
        let retry = l.check(ip("1.1.1.1"), None).await.unwrap_err();
        assert!(retry > 0 && retry <= 300, "retry {retry}");
        // A different address is unaffected.
        assert!(l.check(ip("2.2.2.2"), None).await.is_ok());
    }

    #[tokio::test]
    async fn locks_an_account_across_ips() {
        let l = limiter(100, 3).await;
        for n in 0..3 {
            l.record_failure(ip(&format!("10.0.0.{n}")), Some("alice"))
                .await;
        }
        assert!(l.check(ip("10.0.0.99"), Some("alice")).await.is_err());
        assert!(l.check(ip("10.0.0.99"), Some("bob")).await.is_ok());
    }

    #[tokio::test]
    async fn clearing_an_account_keeps_the_ip_count() {
        let l = limiter(2, 2).await;
        l.record_failure(ip("1.1.1.1"), Some("alice")).await;
        l.record_failure(ip("1.1.1.1"), Some("alice")).await;
        l.clear_account("alice").await;

        // The account is free again from another address...
        assert!(l.check(ip("2.2.2.2"), Some("alice")).await.is_ok());
        // ...but the address that failed is still locked, whatever account
        // it targets.
        assert!(l.check(ip("1.1.1.1"), Some("bob")).await.is_err());
    }
}
