use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Semaphore;

use crate::errors::AppError;

/// How long a request waits for a hashing slot before giving up with 503.
/// At a few hundred ms per hash, this covers a deep queue; past it, the
/// client is better served by an error than by hanging.
const MAX_HASH_WAIT: Duration = Duration::from_secs(10);

/// Runs Argon2 work off the async runtime, a bounded number at a time.
///
/// Each Argon2 call deliberately burns ~100+ ms of CPU and ~19 MiB of memory.
/// Run inline in a handler, it holds a tokio worker for that long, so a login
/// surge stalls every other request and socket on the same workers. Here the
/// work moves to the blocking pool, and a semaphore caps how many hashes run
/// at once so a surge queues instead of saturating the CPU.
struct HashGate {
    permits: Semaphore,
    max_wait: Duration,
}

impl HashGate {
    fn new(permits: usize, max_wait: Duration) -> Self {
        Self {
            permits: Semaphore::new(permits),
            max_wait,
        }
    }

    async fn run<T, F>(&self, work: F) -> Result<T, AppError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = tokio::time::timeout(self.max_wait, self.permits.acquire())
            .await
            .map_err(|_| AppError::ServiceUnavailable("Server is busy, try again shortly".into()))?
            .map_err(|e| AppError::Internal(format!("hash gate closed: {e}")))?;
        let result = tokio::task::spawn_blocking(work)
            .await
            .map_err(|e| AppError::Internal(format!("hashing task failed: {e}")));
        drop(permit);
        result
    }
}

static HASH_GATE: OnceLock<HashGate> = OnceLock::new();

/// One slot per core: more concurrent hashes than cores only adds memory
/// and contention without finishing any of them sooner.
fn hash_gate() -> &'static HashGate {
    HASH_GATE.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        HashGate::new(cores, MAX_HASH_WAIT)
    })
}

/// [`hash_password`] through the hash gate. Request handlers must use this
/// (and the other `_gated` functions), never the synchronous versions.
pub async fn hash_password_gated(plain: String) -> Result<String, AppError> {
    hash_gate().run(move || hash_password(&plain)).await?
}

/// [`verify_password`] through the hash gate.
pub async fn verify_password_gated(plain: String, hash: String) -> Result<bool, AppError> {
    hash_gate()
        .run(move || verify_password(&plain, &hash))
        .await?
}

/// [`verify_password_against_dummy`] through the hash gate. It queues
/// exactly like a real verification, so waiting for a slot doesn't
/// reveal whether an account exists either.
pub async fn verify_password_against_dummy_gated(plain: String) -> Result<(), AppError> {
    hash_gate()
        .run(move || verify_password_against_dummy(&plain))
        .await
}

pub fn hash_password(plain: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))
}

pub fn verify_password(plain: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("invalid password hash format: {e}")))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

fn dummy_hash() -> &'static str {
    DUMMY_HASH.get_or_init(|| {
        hash_password("dummy-password-for-timing-equalization")
            .expect("dummy password hashing must succeed at startup")
    })
}

/// Run an Argon2 verification against a fixed dummy hash, discarding the
/// result. Used in the login handler when the username/email doesn't exist —
/// without this call, the "user-not-found" path returns in microseconds
/// while the "user-found-wrong-password" path takes ~100 ms, letting an
/// attacker enumerate valid accounts by timing.
pub fn verify_password_against_dummy(plain: &str) {
    let _ = verify_password(plain, dummy_hash());
}

/// Pre-compute the dummy hash at startup so the first cold login request
/// doesn't leak the initialization cost.
pub fn warm_dummy_hash() {
    let _ = dummy_hash();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash).unwrap());
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("correct-password").unwrap();
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn different_calls_produce_different_hashes() {
        let h1 = hash_password("same-password").unwrap();
        let h2 = hash_password("same-password").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_contains_argon2id_identifier() {
        let hash = hash_password("test").unwrap();
        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn invalid_hash_format_returns_error() {
        let result = verify_password("test", "not-a-valid-hash");
        assert!(result.is_err());
    }

    #[test]
    fn dummy_hash_is_stable_across_calls() {
        let a = dummy_hash();
        let b = dummy_hash();
        assert_eq!(a, b);
        assert!(a.starts_with("$argon2id$"));
    }

    #[test]
    fn verify_against_dummy_does_not_panic() {
        verify_password_against_dummy("anything");
        verify_password_against_dummy("");
        verify_password_against_dummy("\0\0\0");
    }

    #[test]
    fn warm_dummy_hash_initializes_lock() {
        warm_dummy_hash();
        let h = dummy_hash();
        assert!(h.starts_with("$argon2id$"));
    }

    #[tokio::test]
    async fn gated_hash_and_verify_roundtrip() {
        let hash = hash_password_gated("hunter2".into()).await.unwrap();
        assert!(verify_password_gated("hunter2".into(), hash.clone())
            .await
            .unwrap());
        assert!(!verify_password_gated("wrong".into(), hash).await.unwrap());
        verify_password_against_dummy_gated("anything".into())
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn gate_never_runs_more_than_its_permits() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let gate = Arc::new(HashGate::new(2, Duration::from_secs(10)));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let (gate, in_flight, peak) = (gate.clone(), in_flight.clone(), peak.clone());
                tokio::spawn(async move {
                    gate.run(move || {
                        let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn gate_gives_up_with_503_after_max_wait() {
        use std::sync::Arc;

        let gate = Arc::new(HashGate::new(1, Duration::from_millis(50)));
        let holder = {
            let gate = gate.clone();
            tokio::spawn(async move {
                gate.run(|| std::thread::sleep(Duration::from_millis(500)))
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;

        let waited = gate.run(|| ()).await;
        assert!(matches!(waited, Err(AppError::ServiceUnavailable(_))));
        holder.await.unwrap().unwrap();
    }

    /// The point of the gate: on a single-threaded runtime, a slow hash must
    /// not stop other tasks. Run inline, the ticker could not finish first.
    #[tokio::test(flavor = "current_thread")]
    async fn gated_work_does_not_block_the_runtime() {
        let gate = HashGate::new(1, Duration::from_secs(10));
        let slow = gate.run(|| std::thread::sleep(Duration::from_millis(300)));
        let ticker = tokio::time::sleep(Duration::from_millis(10));

        tokio::select! {
            _ = slow => panic!("blocking work finished before the 10 ms ticker"),
            () = ticker => {}
        }
    }
}
