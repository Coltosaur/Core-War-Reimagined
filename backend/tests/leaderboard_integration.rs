//! HTTP integration tests for `/api/leaderboard`: public access, ordering,
//! rank numbering, pagination defaults/clamping, and response shape.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{auth, leaderboard};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

use common::{get_with_cookies, post_json, test_state};

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/leaderboard", get(leaderboard::handlers::leaderboard))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

async fn get_json(router: &Router, path: &str) -> (StatusCode, Value) {
    let req = Request::get(path).body(Body::empty()).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// Seeds a user with an explicit rating, created `secs_ago` seconds in the past.
async fn seed_user(pool: &PgPool, username: &str, rating: i32, secs_ago: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, rating, created_at) \
         VALUES ($1, 'x', $2, now() - make_interval(secs => $3)) RETURNING id",
    )
    .bind(username)
    .bind(rating)
    .bind(secs_ago as f64)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn usernames(json: &Value) -> Vec<&str> {
    json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["username"].as_str().unwrap())
        .collect()
}

fn ranks(json: &Value) -> Vec<i64> {
    json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["rank"].as_i64().unwrap())
        .collect()
}

fn keys(v: &Value) -> Vec<&str> {
    let mut k: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    k.sort_unstable();
    k
}

#[sqlx::test]
async fn empty_leaderboard_is_public_with_default_paging(pool: PgPool) {
    let router = app(pool);
    let (status, json) = get_json(&router, "/api/leaderboard").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), ["entries", "page", "per_page", "total"]);
    assert_eq!(json["entries"], json!([]));
    assert_eq!(json["total"], 0);
    assert_eq!(json["page"], 1);
    assert_eq!(json["per_page"], 50);
}

#[sqlx::test]
async fn invalid_cookie_is_ignored(pool: PgPool) {
    let router = app(pool.clone());
    seed_user(&pool, "alice", 1200, 0).await;
    let resp = router
        .oneshot(get_with_cookies("/api/leaderboard", "access_token=garbage"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test]
async fn registered_user_appears_with_default_rating_and_entry_shape(pool: PgPool) {
    let router = app(pool);
    let resp = router
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            &json!({"username": "alice", "email": "a@example.com", "password": "password1234"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let user_id = serde_json::from_slice::<Value>(&body).unwrap()["user_id"].clone();

    let (_, json) = get_json(&router, "/api/leaderboard").await;
    let entry = &json["entries"][0];
    // No email / password_hash leakage.
    assert_eq!(
        keys(entry),
        ["created_at", "rank", "rating", "user_id", "username"]
    );
    assert_eq!(entry["rank"], 1);
    assert_eq!(entry["user_id"], user_id);
    assert_eq!(entry["username"], "alice");
    assert_eq!(entry["rating"], 1200);
    assert!(entry["created_at"].is_string());
}

#[sqlx::test]
async fn ordered_by_rating_desc_then_oldest_first(pool: PgPool) {
    let router = app(pool.clone());
    seed_user(&pool, "low", 1000, 50).await;
    seed_user(&pool, "tie_newer", 1300, 10).await;
    seed_user(&pool, "top", 1500, 0).await;
    seed_user(&pool, "tie_older", 1300, 40).await;

    let (_, json) = get_json(&router, "/api/leaderboard").await;
    assert_eq!(usernames(&json), ["top", "tie_older", "tie_newer", "low"]);
    // Tied ratings still get distinct, sequential ranks (ROW_NUMBER).
    assert_eq!(ranks(&json), [1, 2, 3, 4]);
    let ratings: Vec<i64> = json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["rating"].as_i64().unwrap())
        .collect();
    assert_eq!(ratings, [1500, 1300, 1300, 1000]);
    assert_eq!(json["total"], 4);
}

#[sqlx::test]
async fn pagination_offsets_ranks_and_clamping(pool: PgPool) {
    let router = app(pool.clone());
    for i in 0..5 {
        seed_user(&pool, &format!("u{i}"), 1500 - i * 10, 0).await;
    }

    // Ranks are global, not per-page.
    let (_, json) = get_json(&router, "/api/leaderboard?page=2&per_page=2").await;
    assert_eq!(usernames(&json), ["u2", "u3"]);
    assert_eq!(ranks(&json), [3, 4]);
    assert_eq!(json["page"], 2);
    assert_eq!(json["per_page"], 2);
    assert_eq!(json["total"], 5);

    let (_, json) = get_json(&router, "/api/leaderboard?page=3&per_page=2").await;
    assert_eq!(usernames(&json), ["u4"]);
    assert_eq!(ranks(&json), [5]);

    let (status, json) = get_json(&router, "/api/leaderboard?page=10&per_page=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["entries"], json!([]));
    assert_eq!(json["total"], 5);
    assert_eq!(json["page"], 10);

    for q in ["page=0", "page=-3"] {
        let (_, json) = get_json(&router, &format!("/api/leaderboard?{q}&per_page=2")).await;
        assert_eq!(json["page"], 1);
        assert_eq!(usernames(&json), ["u0", "u1"]);
    }

    let (_, json) = get_json(&router, "/api/leaderboard?per_page=0").await;
    assert_eq!(json["per_page"], 1);
    assert_eq!(usernames(&json), ["u0"]);
    let (_, json) = get_json(&router, "/api/leaderboard?per_page=-1").await;
    assert_eq!(json["per_page"], 1);
    let (_, json) = get_json(&router, "/api/leaderboard?per_page=101").await;
    assert_eq!(json["per_page"], 100);
    assert_eq!(usernames(&json).len(), 5);
}

#[sqlx::test]
async fn non_numeric_query_is_400(pool: PgPool) {
    let router = app(pool);
    for q in ["page=abc", "per_page=1.5"] {
        let (status, _) = get_json(&router, &format!("/api/leaderboard?{q}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{q}");
    }
}
