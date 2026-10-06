//! Helpers shared by the integration test binaries. Each binary uses only
//! some of them, hence the `dead_code` allow.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::Request;
use core_war_backend::{AppConfig, AppState};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;

pub const FRONTEND_URL: &str = "http://localhost:5173";
pub const JWT_SECRET: &[u8] = b"integration-test-secret-that-is-at-least-32-bytes!!";

pub fn test_state(pool: PgPool) -> AppState {
    AppState {
        db: pool,
        config: AppConfig {
            frontend_url: FRONTEND_URL.into(),
            jwt_secret: JWT_SECRET.to_vec(),
            trusted_proxies: vec![],
        },
    }
}

// --- Request builders ---

pub fn post_json(path: &str, body: &Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("origin", FRONTEND_URL)
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn post_json_with_cookies(path: &str, body: &Value, cookies: &str) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("origin", FRONTEND_URL)
        .header("cookie", cookies)
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub fn get_with_cookies(path: &str, cookies: &str) -> Request<Body> {
    Request::get(path)
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

// --- Response helpers ---

pub fn extract_cookies(headers: &axum::http::HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for header in headers.get_all("set-cookie") {
        if let Ok(s) = header.to_str() {
            if let Some(cookie_part) = s.split(';').next() {
                if let Some((name, value)) = cookie_part.split_once('=') {
                    map.insert(name.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }
    map
}
