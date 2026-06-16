use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use std::sync::OnceLock;

use crate::errors::AppError;

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
}
