//! In-process fallback for the Redis-backed limiters (#132).
//!
//! The limiters in `rate_limit` and `failure_limit` keep their windows in
//! Redis. When Redis is unreachable they used to allow every request, which
//! silently switched off brute-force protection on login for the length of
//! the outage. Now they fall back to the windows here: the same limits, kept
//! in this process's memory.
//!
//! **This is a per-instance degraded mode, not an equivalent of Redis.**
//! Each backend process has its own windows, so behind a load balancer an
//! attacker who lands on different instances gets a separate bucket on each.
//! Counts also start from zero when Redis goes down (the Redis windows
//! can't be read) and when the process restarts (nothing here is written to
//! disk; Redis is the persistence layer). Today there is one instance, so
//! during an outage the limits hold as written.
//!
//! "Down" includes slow. A Redis call that takes longer than
//! `REDIS_TIMEOUT` is treated exactly like a failed one, so a latency spike
//! also starts these windows from zero: a key locked in Redis reads as
//! unlocked until Redis answers in time again, though the limits here still
//! apply in the meantime. That makes `REDIS_TIMEOUT` a security setting,
//! not just a latency one.
//!
//! Failures recorded here during an outage keep counting after Redis comes
//! back: callers check these windows as well as Redis until they age out,
//! so a Redis blip can't be used to reset a lockout. The two sides are not
//! summed, though, so for one window after recovery a key can get up to its
//! limit on each side.
//!
//! In the other direction, one request can be counted on both sides. A
//! timeout only means the reply didn't arrive in time; the script may still
//! have run in Redis, and the request is then counted here as well. This
//! only ever makes the limits stricter, so it is left as is.
//!
//! A successful login clears its account's failures. If Redis can't be
//! reached to do that, the clear is remembered with its time and sent before
//! Redis is next checked for that account, so a lock from before the outage
//! doesn't come back with Redis. The late clear removes only failures from
//! up to that time, so one counted in the meantime survives it.
//!
//! Memory is bounded. Each window holds at most its limit in timestamps, and
//! each limiter holds at most `DEFAULT_CAPACITY` keys. When full, expired
//! windows go first, then windows under their limit, least recently active
//! first. Windows at their limit (locked keys) go only when every key is
//! locked.
//!
//! **That last step is a deliberate memory-over-security tradeoff.** An
//! attacker who fills the table with locked keys can push out another
//! locked key, which then starts again from zero. Keys are only created by
//! counted requests and the per-IP limits bound how fast one source creates
//! them, so this takes on the order of a thousand addresses, each spending
//! its full quota; and it only lasts until Redis is back. The alternative,
//! an unbounded map, would let the same attacker exhaust the process's
//! memory instead.

use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// How long a limiter waits on Redis before using the fallback. Without a
/// bound, a request during an outage would wait out the connection
/// manager's reconnect backoff, which takes seconds.
///
/// Lowering this trades security for latency: every timeout drops the
/// limiters to windows that start from zero (see the module docs).
const REDIS_TIMEOUT: Duration = Duration::from_millis(500);

/// Keys per limiter. At a few hundred bytes per key, the cap keeps each
/// limiter's fallback to a few MiB.
const DEFAULT_CAPACITY: usize = 20_000;

/// Runs a Redis call with `REDIS_TIMEOUT`. `Err` carries a description for
/// the log.
pub(crate) async fn redis_bounded<T>(
    call: impl Future<Output = redis::RedisResult<T>>,
) -> Result<T, String> {
    match tokio::time::timeout(REDIS_TIMEOUT, call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("no response within {REDIS_TIMEOUT:?}")),
    }
}

/// Sliding windows of timestamps (in seconds), one per key, with the same
/// semantics as the limiters' Lua scripts.
pub(crate) struct LocalWindows {
    name: String,
    window_secs: u64,
    capacity: usize,
    state: Mutex<State>,
    degraded: AtomicBool,
}

#[derive(Default)]
struct State {
    windows: HashMap<String, Window>,
    /// Keys whose Redis clear failed and still has to be sent, with the
    /// time of the clear: only failures up to then are removed.
    pending_clears: HashMap<String, u64>,
}

#[derive(Default)]
struct Window {
    stamps: VecDeque<u64>,
    /// The limit this key was last counted against.
    max: u32,
}

impl Window {
    fn locked(&self) -> bool {
        self.stamps.len() >= self.max as usize
    }

    fn last_active(&self) -> u64 {
        self.stamps.back().copied().unwrap_or(0)
    }
}

impl LocalWindows {
    pub(crate) fn new(name: &str, window_secs: u64) -> Self {
        Self::with_capacity(name, window_secs, DEFAULT_CAPACITY)
    }

    fn with_capacity(name: &str, window_secs: u64, capacity: usize) -> Self {
        Self {
            name: name.to_string(),
            window_secs,
            capacity,
            state: Mutex::default(),
            degraded: AtomicBool::new(false),
        }
    }

    /// The seconds until every key is under its limit, or `None` if none of
    /// them is at it. `keys` pairs each key with its limit.
    pub(crate) fn blocked(&self, keys: &[(String, u32)], now: u64) -> Option<u64> {
        let windows = &mut self.lock().windows;
        let mut retry = None;
        for (key, max) in keys {
            let Some(window) = windows.get_mut(key) else {
                continue;
            };
            self.prune(&mut window.stamps, now);
            if window.stamps.is_empty() {
                windows.remove(key);
            } else if let Some(wait) = self.wait_if_full(&window.stamps, *max, now) {
                // `None < Some(_)` for `Option`, so this keeps the longest wait.
                retry = retry.max(Some(wait));
            }
        }
        retry
    }

    /// Adds one entry at `now` to every key.
    pub(crate) fn record(&self, keys: &[(String, u32)], now: u64) {
        let windows = &mut self.lock().windows;
        for (key, max) in keys {
            self.push(windows, key, *max, now);
        }
    }

    /// Counts the request unless the key is already at its limit, in one
    /// step, like the per-request limiter's script. `Err` is the seconds to
    /// wait.
    pub(crate) fn check_and_record(&self, key: &str, max: u32, now: u64) -> Result<(), u64> {
        let windows = &mut self.lock().windows;
        if let Some(window) = windows.get_mut(key) {
            self.prune(&mut window.stamps, now);
            if let Some(wait) = self.wait_if_full(&window.stamps, max, now) {
                return Err(wait);
            }
        }
        self.push(windows, key, max, now);
        Ok(())
    }

    pub(crate) fn clear(&self, key: &str) {
        self.lock().windows.remove(key);
    }

    /// Remembers that Redis still holds failures for `key` up to `cleared_at`
    /// that should have been cleared. Bounded like the windows: past the cap
    /// the clear is dropped, and the Redis lock simply ages out.
    pub(crate) fn defer_clear(&self, key: &str, cleared_at: u64) {
        let pending = &mut self.lock().pending_clears;
        if let Some(at) = pending.get_mut(key) {
            *at = (*at).max(cleared_at);
        } else if pending.len() < self.capacity {
            pending.insert(key.to_string(), cleared_at);
        } else {
            tracing::warn!(
                limiter = %self.name,
                "too many pending Redis clears; dropping one, which will age out instead"
            );
        }
    }

    /// Takes the deferred clears among `keys`, with their times. The caller
    /// sends them to Redis and hands back any that fail with `defer_clear`.
    /// Taking them means only one request sends each.
    pub(crate) fn take_pending_clears(&self, keys: &[(String, u32)]) -> Vec<(String, u64)> {
        let pending = &mut self.lock().pending_clears;
        if pending.is_empty() {
            return Vec::new();
        }
        keys.iter()
            .filter_map(|(key, _)| pending.remove_entry(key))
            .collect()
    }

    /// Call when a Redis operation failed and the fallback took over. Logs
    /// the switch once, not once per request.
    pub(crate) fn redis_failed(&self, operation: &str, error: &dyn Display) {
        if !self.degraded.swap(true, Ordering::Relaxed) {
            tracing::error!(
                limiter = %self.name,
                %operation,
                %error,
                "Redis unavailable: rate limiting falls back to in-process \
                 limits. This is per-instance degraded mode; with more than \
                 one backend instance, each keeps its own counts."
            );
        } else {
            tracing::debug!(limiter = %self.name, %operation, %error, "Redis still unavailable");
        }
    }

    /// Call when a Redis operation succeeded.
    pub(crate) fn redis_ok(&self) {
        if self.degraded.swap(false, Ordering::Relaxed) {
            tracing::warn!(
                limiter = %self.name,
                "Redis reachable again: rate limiting is back on Redis"
            );
        }
    }

    pub(crate) fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // Every critical section leaves the state consistent, so a panic in
        // another holder doesn't make the data unusable.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Drops entries at or before `now - window`, as the scripts'
    /// `ZREMRANGEBYSCORE key -inf cutoff` does.
    fn prune(&self, window: &mut VecDeque<u64>, now: u64) {
        let Some(cutoff) = now.checked_sub(self.window_secs) else {
            return;
        };
        while window.front().is_some_and(|&t| t <= cutoff) {
            window.pop_front();
        }
    }

    /// When the window is at `max`, the seconds until its oldest entry ages
    /// out, as the scripts compute it.
    fn wait_if_full(&self, window: &VecDeque<u64>, max: u32, now: u64) -> Option<u64> {
        if window.len() < max as usize {
            return None;
        }
        let oldest = *window.front()?;
        Some(self.window_secs.saturating_sub(now.saturating_sub(oldest)))
    }

    fn push(&self, windows: &mut HashMap<String, Window>, key: &str, max: u32, now: u64) {
        if !windows.contains_key(key) {
            self.make_room(windows, now);
        }
        let window = windows.entry(key.to_string()).or_default();
        window.max = max.max(1);
        self.prune(&mut window.stamps, now);
        window.stamps.push_back(now);
        // Only the newest `max` entries matter: the window unblocks when the
        // oldest of them ages out.
        while window.stamps.len() > window.max as usize {
            window.stamps.pop_front();
        }
    }

    fn make_room(&self, windows: &mut HashMap<String, Window>, now: u64) {
        if windows.len() < self.capacity {
            return;
        }
        if let Some(cutoff) = now.checked_sub(self.window_secs) {
            windows.retain(|_, w| w.stamps.back().is_some_and(|&t| t > cutoff));
        }
        if windows.len() < self.capacity {
            return;
        }
        // Still full of live windows. Evict an eighth in one go, so a
        // sustained flood pays for this scan rarely: unlocked keys first,
        // least recently active first. A locked key stops being counted
        // (it's refused before it can fail again), so by activity alone it
        // would be the first to go.
        let evict = (windows.len() / 8).max(1);
        let mut order: Vec<((bool, u64), String)> = windows
            .iter()
            .map(|(key, w)| ((w.locked(), w.last_active()), key.clone()))
            .collect();
        order.select_nth_unstable_by_key(evict - 1, |(rank, _)| *rank);
        let evicted_locked = order[..evict].iter().filter(|((l, _), _)| *l).count();
        for (_, key) in &order[..evict] {
            windows.remove(key);
        }
        tracing::warn!(
            limiter = %self.name,
            evicted = evict,
            evicted_locked,
            capacity = self.capacity,
            "in-process rate-limit fallback is full; evicted keys"
        );
    }
}

/// A Redis connection that tests can cut and restore, by routing it through
/// a local TCP relay in front of the real Redis.
#[cfg(test)]
pub(crate) mod test_relay {
    use redis::aio::ConnectionManager;
    use std::sync::{Arc, Mutex};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::AbortHandle;

    pub(crate) struct RedisRelay {
        port: u16,
        tasks: Arc<Mutex<Vec<AbortHandle>>>,
    }

    impl RedisRelay {
        /// Starts the relay and returns a connection manager that talks to
        /// Redis through it.
        pub(crate) async fn start() -> (Self, ConnectionManager) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let relay = Self {
                port: listener.local_addr().unwrap().port(),
                tasks: Arc::default(),
            };
            relay.serve(listener);
            let url = format!("redis://127.0.0.1:{}", relay.port);
            let conn = ConnectionManager::new(redis::Client::open(url).unwrap())
                .await
                .unwrap();
            (relay, conn)
        }

        /// Closes every relayed connection and refuses new ones.
        pub(crate) fn cut(&self) {
            for task in self.tasks.lock().unwrap().drain(..) {
                task.abort();
            }
        }

        /// Accepts connections again, on the same port.
        pub(crate) async fn restore(&self) {
            let listener = TcpListener::bind(("127.0.0.1", self.port)).await.unwrap();
            self.serve(listener);
        }

        fn serve(&self, listener: TcpListener) {
            let tasks = self.tasks.clone();
            let accept = tokio::spawn(async move {
                while let Ok((mut client, _)) = listener.accept().await {
                    let pipe = tokio::spawn(async move {
                        let mut redis = TcpStream::connect(redis_addr()).await.unwrap();
                        let _ = tokio::io::copy_bidirectional(&mut client, &mut redis).await;
                    });
                    tasks.lock().unwrap().push(pipe.abort_handle());
                }
            });
            self.tasks.lock().unwrap().push(accept.abort_handle());
        }
    }

    impl Drop for RedisRelay {
        fn drop(&mut self) {
            self.cut();
        }
    }

    fn redis_addr() -> String {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
        let rest = url.trim_start_matches("redis://");
        let host_port = rest.rsplit('@').next().unwrap_or(rest);
        host_port.split('/').next().unwrap_or(host_port).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[(&str, u32)]) -> Vec<(String, u32)> {
        list.iter().map(|(k, m)| (k.to_string(), *m)).collect()
    }

    #[test]
    fn blocks_at_the_limit_and_reports_the_wait() {
        let w = LocalWindows::new("t", 300);
        let k = keys(&[("ip", 3)]);
        for t in [100, 110, 120] {
            assert_eq!(w.blocked(&k, t), None);
            w.record(&k, t);
        }
        // The oldest entry (100) ages out at 400.
        assert_eq!(w.blocked(&k, 150), Some(250));
    }

    #[test]
    fn entries_age_out_like_the_scripts() {
        let w = LocalWindows::new("t", 300);
        let k = keys(&[("ip", 1)]);
        w.record(&k, 100);
        assert!(w.blocked(&k, 399).is_some());
        // ZREMRANGEBYSCORE -inf (now - window) is inclusive.
        assert_eq!(w.blocked(&k, 400), None);
    }

    #[test]
    fn the_longest_wait_across_keys_wins() {
        let w = LocalWindows::new("t", 300);
        w.record(&keys(&[("ip", 1)]), 100);
        w.record(&keys(&[("account", 1)]), 200);
        let both = keys(&[("ip", 1), ("account", 1)]);
        assert_eq!(w.blocked(&both, 250), Some(250));
        // A key under its limit doesn't block.
        assert_eq!(w.blocked(&keys(&[("ip", 2)]), 250), None);
    }

    #[test]
    fn check_and_record_refuses_without_counting() {
        let w = LocalWindows::new("t", 60);
        assert!(w.check_and_record("ip", 2, 0).is_ok());
        assert!(w.check_and_record("ip", 2, 10).is_ok());
        assert_eq!(w.check_and_record("ip", 2, 20), Err(40));
        // The refused request wasn't counted: the window frees when the
        // first entry ages out, not later.
        assert!(w.check_and_record("ip", 2, 60).is_ok());
    }

    #[test]
    fn clear_removes_one_key() {
        let w = LocalWindows::new("t", 300);
        w.record(&keys(&[("ip", 1), ("account", 1)]), 0);
        w.clear("account");
        assert_eq!(w.blocked(&keys(&[("account", 1)]), 1), None);
        assert!(w.blocked(&keys(&[("ip", 1)]), 1).is_some());
    }

    #[test]
    fn a_window_never_holds_more_than_its_limit() {
        let w = LocalWindows::new("t", 300);
        let k = keys(&[("ip", 3)]);
        for t in 0..100 {
            w.record(&k, t);
        }
        assert_eq!(w.lock().windows["ip"].stamps.len(), 3);
        // The newest three remain, so the wait runs from 97.
        assert_eq!(w.blocked(&k, 100), Some(297));
    }

    #[test]
    fn capacity_evicts_expired_keys_first() {
        let w = LocalWindows::with_capacity("t", 100, 2);
        w.record(&keys(&[("old", 5)]), 0);
        w.record(&keys(&[("live", 5)]), 150);
        w.record(&keys(&[("new", 5)]), 160);
        let windows = &w.lock().windows;
        assert_eq!(windows.len(), 2);
        assert!(windows.contains_key("live") && windows.contains_key("new"));
    }

    #[test]
    fn capacity_then_evicts_the_least_recently_active() {
        let w = LocalWindows::with_capacity("t", 1000, 3);
        w.record(&keys(&[("a", 5)]), 10);
        w.record(&keys(&[("b", 5)]), 20);
        w.record(&keys(&[("c", 5)]), 30);
        // `a` is touched again, so `b` is now the least recently active.
        w.record(&keys(&[("a", 5)]), 40);
        w.record(&keys(&[("d", 5)]), 50);
        let windows = &w.lock().windows;
        assert_eq!(windows.len(), 3);
        assert!(!windows.contains_key("b"));
    }

    #[test]
    fn locked_keys_outlast_unlocked_ones_when_full() {
        let w = LocalWindows::with_capacity("t", 1000, 3);
        // The target is locked early and, being refused, never counted
        // again: by activity alone it would go first.
        w.record(&keys(&[("target", 1)]), 10);
        w.record(&keys(&[("noise-1", 5)]), 20);
        w.record(&keys(&[("noise-2", 5)]), 30);
        w.record(&keys(&[("noise-3", 5)]), 40);
        assert!(w.blocked(&keys(&[("target", 1)]), 50).is_some());
        assert!(!w.lock().windows.contains_key("noise-1"));
    }

    #[test]
    fn a_table_full_of_locked_keys_evicts_a_locked_key() {
        // The documented tradeoff: with every key locked, the least
        // recently active lock is lost rather than memory growing.
        let w = LocalWindows::with_capacity("t", 1000, 3);
        w.record(&keys(&[("target", 1)]), 10);
        w.record(&keys(&[("flood-1", 1)]), 20);
        w.record(&keys(&[("flood-2", 1)]), 30);
        w.record(&keys(&[("flood-3", 1)]), 40);
        assert_eq!(w.blocked(&keys(&[("target", 1)]), 50), None);
        assert_eq!(w.lock().windows.len(), 3);
    }

    #[test]
    fn pending_clears_are_taken_once() {
        let w = LocalWindows::new("t", 60);
        w.defer_clear("alice", 100);
        let both = keys(&[("ip", 5), ("alice", 5)]);
        assert_eq!(
            w.take_pending_clears(&both),
            vec![("alice".to_string(), 100)]
        );
        // A concurrent request doesn't get it again.
        assert!(w.take_pending_clears(&both).is_empty());
        // Unrelated keys never pick it up.
        w.defer_clear("alice", 100);
        assert!(w.take_pending_clears(&keys(&[("bob", 5)])).is_empty());
    }

    #[test]
    fn a_repeated_clear_keeps_the_latest_time() {
        let w = LocalWindows::new("t", 60);
        w.defer_clear("alice", 200);
        // A failed resend hands back the original, earlier time.
        w.defer_clear("alice", 100);
        assert_eq!(
            w.take_pending_clears(&keys(&[("alice", 5)])),
            vec![("alice".to_string(), 200)]
        );
    }

    #[test]
    fn degraded_flag_follows_redis() {
        let w = LocalWindows::new("t", 60);
        assert!(!w.is_degraded());
        w.redis_failed("check", &"connection refused");
        w.redis_failed("check", &"connection refused");
        assert!(w.is_degraded());
        w.redis_ok();
        assert!(!w.is_degraded());
    }
}
