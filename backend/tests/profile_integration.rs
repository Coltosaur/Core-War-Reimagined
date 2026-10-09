//! HTTP integration tests for `/api/profile` (own profile, auth required) and
//! `/api/users/:username` (public profile, auth optional).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{auth, profile};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

use common::{extract_cookies, get_with_cookies, post_json, test_state};

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/profile", get(profile::handlers::me))
        .route(
            "/api/users/:username",
            get(profile::handlers::public_profile),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

fn get_no_auth(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

/// Registers a user over HTTP; returns (user_id, cookie header).
async fn register(router: &Router, username: &str) -> (Uuid, String) {
    let resp = router
        .clone()
        .oneshot(post_json(
            "/api/auth/register",
            &json!({"username": username, "password": "password1234"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let cookies = extract_cookies(resp.headers());
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let id = json["user_id"].as_str().unwrap().parse().unwrap();
    (id, format!("access_token={}", cookies["access_token"]))
}

async fn insert_warrior(pool: &PgPool, user_id: Uuid, name: &str, secs_ago: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO warriors (user_id, name, source, updated_at) \
         VALUES ($1, $2, 'MOV.I $0, $1', now() - make_interval(secs => $3)) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(secs_ago as f64)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_match(
    pool: &PgPool,
    (red_w, red_u): (Uuid, Uuid),
    (blue_w, blue_u): (Uuid, Uuid),
    result: &str,
) {
    sqlx::query(
        "INSERT INTO matches (red_warrior_id, blue_warrior_id, red_user_id, blue_user_id, \
           result, steps_taken) VALUES ($1, $2, $3, $4, $5, 10)",
    )
    .bind(red_w)
    .bind(blue_w)
    .bind(red_u)
    .bind(blue_u)
    .bind(result)
    .execute(pool)
    .await
    .unwrap();
}

fn keys(v: &Value) -> Vec<&str> {
    let mut k: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    k.sort_unstable();
    k
}

fn stats(json: &Value) -> [i64; 5] {
    ["warrior_count", "match_count", "wins", "losses", "ties"].map(|k| json[k].as_i64().unwrap())
}

const PROFILE_KEYS: [&str; 8] = [
    "created_at",
    "losses",
    "match_count",
    "ties",
    "user_id",
    "username",
    "warrior_count",
    "wins",
];

/// alice: 2 warriors. Matches involving alice: win as red, win as blue,
/// loss as red, tie, all_dead. Plus one bob-vs-carol match she isn't in.
async fn seed_alice_history(pool: &PgPool, alice: Uuid, bob: Uuid, carol: Uuid) -> (Uuid, Uuid) {
    let a1 = insert_warrior(pool, alice, "Older", 60).await;
    let a2 = insert_warrior(pool, alice, "Newer", 0).await;
    let b = insert_warrior(pool, bob, "Bob W", 0).await;
    let c = insert_warrior(pool, carol, "Carol W", 0).await;
    insert_match(pool, (a1, alice), (b, bob), "red_win").await;
    insert_match(pool, (b, bob), (a2, alice), "blue_win").await;
    insert_match(pool, (a1, alice), (b, bob), "blue_win").await;
    insert_match(pool, (b, bob), (a1, alice), "tie").await;
    insert_match(pool, (a2, alice), (b, bob), "all_dead").await;
    insert_match(pool, (b, bob), (c, carol), "red_win").await;
    (a1, a2)
}

// =============================================================================
// /api/profile
// =============================================================================

#[sqlx::test]
async fn own_profile_requires_auth(pool: PgPool) {
    let router = app(pool);
    for req in [
        get_no_auth("/api/profile"),
        get_with_cookies("/api/profile", "access_token=garbage"),
    ] {
        let (status, json) = send(&router, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(json["error"].is_string());
    }
}

#[sqlx::test]
async fn own_profile_fresh_user_is_all_zero(pool: PgPool) {
    let router = app(pool);
    let (alice, cookies) = register(&router, "alice").await;

    let (status, json) = send(&router, get_with_cookies("/api/profile", &cookies)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), PROFILE_KEYS);
    assert_eq!(json["user_id"], alice.to_string());
    assert_eq!(json["username"], "alice");
    assert!(json["created_at"].is_string());
    assert_eq!(stats(&json), [0, 0, 0, 0, 0]);
}

#[sqlx::test]
async fn own_profile_counts_wins_losses_ties_across_both_sides(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let (carol, _) = register(&router, "carol").await;
    seed_alice_history(&pool, alice, bob, carol).await;

    let (status, json) = send(&router, get_with_cookies("/api/profile", &cookies)).await;
    assert_eq!(status, StatusCode::OK);
    // all_dead counts as a tie.
    assert_eq!(stats(&json), [2, 5, 2, 1, 2]);
}

#[sqlx::test]
async fn own_profile_stats_db_error_is_500_not_zeroed(pool: PgPool) {
    let router = app(pool.clone());
    let (_, cookies) = register(&router, "alice").await;
    // The users and warriors lookups still succeed; only the stats query fails.
    sqlx::query("DROP TABLE matches CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    for req in [
        get_with_cookies("/api/profile", &cookies),
        get_no_auth("/api/users/alice"),
    ] {
        let (status, json) = send(&router, req).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json, json!({"error": "Internal server error"}));
    }
}

// =============================================================================
// /api/users/:username
// =============================================================================

#[sqlx::test]
async fn public_profile_shape_stats_and_warriors(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, _) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let (carol, _) = register(&router, "carol").await;
    let (older, newer) = seed_alice_history(&pool, alice, bob, carol).await;

    let (status, json) = send(&router, get_no_auth("/api/users/alice")).await;
    assert_eq!(status, StatusCode::OK);
    let mut expected: Vec<&str> = PROFILE_KEYS.into();
    expected.push("warriors");
    expected.sort_unstable();
    assert_eq!(keys(&json), expected);
    assert_eq!(json["user_id"], alice.to_string());
    assert_eq!(json["username"], "alice");
    assert!(json["created_at"].is_string());
    assert_eq!(stats(&json), [2, 5, 2, 1, 2]);

    // Most recently updated first; source is never exposed.
    let warriors = json["warriors"].as_array().unwrap();
    assert_eq!(warriors.len(), 2);
    assert_eq!(warriors[0]["id"], newer.to_string());
    assert_eq!(warriors[0]["name"], "Newer");
    assert_eq!(warriors[1]["id"], older.to_string());
    for w in warriors {
        assert_eq!(keys(w), ["created_at", "id", "name", "updated_at"]);
    }
}

#[sqlx::test]
async fn public_profile_matches_own_profile_stats(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let (carol, _) = register(&router, "carol").await;
    seed_alice_history(&pool, alice, bob, carol).await;

    let (_, own) = send(&router, get_with_cookies("/api/profile", &cookies)).await;
    let (_, public) = send(&router, get_no_auth("/api/users/alice")).await;
    for k in PROFILE_KEYS {
        assert_eq!(own[k], public[k], "{k}");
    }
}

#[sqlx::test]
async fn public_profile_auth_is_optional(pool: PgPool) {
    let router = app(pool);
    let (_, cookies) = register(&router, "alice").await;
    register(&router, "bob").await;

    for c in [cookies.as_str(), "access_token=garbage"] {
        let (status, json) = send(&router, get_with_cookies("/api/users/bob", c)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["username"], "bob");
        assert_eq!(json["warriors"], json!([]));
        assert_eq!(stats(&json), [0, 0, 0, 0, 0]);
    }
}

#[sqlx::test]
async fn public_profile_unknown_user_is_404(pool: PgPool) {
    let router = app(pool);
    let (status, json) = send(&router, get_no_auth("/api/users/nobody")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json, json!({"error": "User not found"}));
}
