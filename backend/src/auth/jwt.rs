use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::AppError;

const ACCESS_TOKEN_DURATION_SECS: i64 = 15 * 60;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub exp: i64,
    pub iat: i64,
}

pub fn encode_access_token(
    user_id: Uuid,
    username: &str,
    secret: &[u8],
) -> Result<String, AppError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp: now + ACCESS_TOKEN_DURATION_SECS,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| AppError::Internal(format!("JWT encoding failed: {e}")))
}

pub fn decode_access_token(token: &str, secret: &[u8]) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Invalid token: {e}")))
}

pub fn generate_refresh_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"this-is-a-test-secret-at-least-32-bytes!";

    #[test]
    fn encode_and_decode_roundtrip() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "testuser", TEST_SECRET).unwrap();
        let claims = decode_access_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, "testuser");
    }

    #[test]
    fn expired_token_rejected() {
        let user_id = Uuid::new_v4();
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            username: "testuser".into(),
            exp: now - 120,
            iat: now - 1020,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET),
        )
        .unwrap();

        let result = decode_access_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_secret_rejected() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "testuser", TEST_SECRET).unwrap();
        let result = decode_access_token(&token, b"wrong-secret-that-is-also-32-bytes!!!");
        assert!(result.is_err());
    }

    #[test]
    fn tampered_token_rejected() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "testuser", TEST_SECRET).unwrap();
        let tampered = format!("{token}x");
        let result = decode_access_token(&tampered, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn claims_contain_correct_expiry_window() {
        let user_id = Uuid::new_v4();
        let before = Utc::now().timestamp();
        let token = encode_access_token(user_id, "testuser", TEST_SECRET).unwrap();
        let claims = decode_access_token(&token, TEST_SECRET).unwrap();
        let after = Utc::now().timestamp();

        let expected_min = before + ACCESS_TOKEN_DURATION_SECS;
        let expected_max = after + ACCESS_TOKEN_DURATION_SECS;
        assert!(claims.exp >= expected_min && claims.exp <= expected_max);
    }

    #[test]
    fn refresh_token_is_64_hex_chars() {
        let token = generate_refresh_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn refresh_tokens_are_unique() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn refresh_token_hash_is_deterministic() {
        let token = generate_refresh_token();
        let h1 = hash_refresh_token(&token);
        let h2 = hash_refresh_token(&token);
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_refresh_tokens_produce_different_hashes() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(hash_refresh_token(&t1), hash_refresh_token(&t2));
    }
}
