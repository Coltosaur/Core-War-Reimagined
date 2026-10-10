//! HTTP integration tests for `/api/matches` (submit, get, list). These pin
//! current behavior — status codes, response shape, pagination — so later
//! refactors of the handlers (pagination dedup, battle loop) can't drift.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{middleware, Router};
use core_war_backend::{auth, matches};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod common;

use common::{extract_cookies, get_with_cookies, post_json, post_json_with_cookies, test_state};

const DWARF: &str = include_str!("../../engine/tests/warriors/dwarf.red");
const MICE_LITE: &str = include_str!("../../engine/tests/warriors/mice_lite.red");

fn app(pool: PgPool) -> Router {
    let state = test_state(pool);
    Router::new()
        .route("/api/auth/register", post(auth::handlers::register))
        .route(
            "/api/matches",
            get(matches::handlers::list).post(matches::handlers::submit),
        )
        .route("/api/matches/:id", get(matches::handlers::get))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
}

async fn send(router: Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
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

async fn insert_warrior(pool: &PgPool, user_id: Uuid, source: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO warriors (user_id, name, source) VALUES ($1, 'W', $2) RETURNING id",
    )
    .bind(user_id)
    .bind(source)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Inserts a match row `secs_ago` seconds in the past so ordering is deterministic.
async fn insert_match(
    pool: &PgPool,
    (red_w, red_u): (Uuid, Uuid),
    (blue_w, blue_u): (Uuid, Uuid),
    secs_ago: i32,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO matches (red_warrior_id, blue_warrior_id, red_user_id, blue_user_id, \
           result, steps_taken, rated, created_at) \
         VALUES ($1, $2, $3, $4, 'tie', 80000, FALSE, now() - make_interval(secs => $5)) \
         RETURNING id",
    )
    .bind(red_w)
    .bind(blue_w)
    .bind(red_u)
    .bind(blue_u)
    .bind(secs_ago as f64)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn submit_body(red: Uuid, blue: Uuid) -> Value {
    json!({"red_warrior_id": red, "blue_warrior_id": blue})
}

fn ids(json: &Value) -> Vec<String> {
    json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect()
}

fn keys(v: &Value) -> Vec<&str> {
    let mut k: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    k.sort_unstable();
    k
}

const MATCH_KEYS: [&str; 11] = [
    "blue_user_id",
    "blue_warrior_id",
    "core_size",
    "created_at",
    "id",
    "max_steps",
    "rated",
    "red_user_id",
    "red_warrior_id",
    "result",
    "steps_taken",
];

// =============================================================================
// Auth
// =============================================================================

#[sqlx::test]
async fn all_match_routes_require_auth(pool: PgPool) {
    let router = app(pool);
    let id = Uuid::new_v4();
    let reqs = [
        post_json("/api/matches", &submit_body(id, id)),
        Request::get("/api/matches").body(Body::empty()).unwrap(),
        Request::get(format!("/api/matches/{id}"))
            .body(Body::empty())
            .unwrap(),
        get_with_cookies("/api/matches", "access_token=garbage"),
    ];
    for req in reqs {
        let (status, json) = send(router.clone(), req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(json["error"].is_string());
    }
}

// =============================================================================
// Submit
// =============================================================================

#[sqlx::test]
async fn submit_runs_battle_and_returns_created_record(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let red = insert_warrior(&pool, alice, DWARF).await;
    let blue = insert_warrior(&pool, bob, MICE_LITE).await;

    let (status, json) = send(
        router,
        post_json_with_cookies("/api/matches", &submit_body(red, blue), &cookies),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(keys(&json), MATCH_KEYS);
    assert_eq!(json["red_warrior_id"], red.to_string());
    assert_eq!(json["blue_warrior_id"], blue.to_string());
    assert_eq!(json["red_user_id"], alice.to_string());
    assert_eq!(json["blue_user_id"], bob.to_string());
    assert_eq!(json["core_size"], 8000);
    assert_eq!(json["max_steps"], 80000);
    assert_eq!(json["result"], "red_win");
    // Decisive match stops early (#87), not at max_steps.
    assert!(json["steps_taken"].as_i64().unwrap() < 80000);
    // Ad-hoc matches never move Elo, so they're unrated (#145).
    assert_eq!(json["rated"], false);

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches WHERE id = $1::uuid")
        .bind(json["id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 1);
}

#[sqlx::test]
async fn submit_allowed_when_caller_owns_only_blue(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, _) = register(&router, "alice").await;
    let (bob, bob_cookies) = register(&router, "bob").await;
    let red = insert_warrior(&pool, alice, DWARF).await;
    let blue = insert_warrior(&pool, bob, MICE_LITE).await;

    let (status, json) = send(
        router,
        post_json_with_cookies("/api/matches", &submit_body(red, blue), &bob_cookies),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["red_user_id"], alice.to_string());
}

#[sqlx::test]
async fn submit_forbidden_when_caller_owns_neither(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, _) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let (_, carol_cookies) = register(&router, "carol").await;
    let red = insert_warrior(&pool, alice, DWARF).await;
    let blue = insert_warrior(&pool, bob, MICE_LITE).await;

    let (status, json) = send(
        router,
        post_json_with_cookies("/api/matches", &submit_body(red, blue), &carol_cookies),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"], "You must own at least one of the warriors");
}

#[sqlx::test]
async fn submit_unknown_warrior_is_404(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let mine = insert_warrior(&pool, alice, DWARF).await;
    let missing = Uuid::new_v4();

    let (status, json) = send(
        router.clone(),
        post_json_with_cookies("/api/matches", &submit_body(missing, mine), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Red warrior not found");

    let (status, json) = send(
        router,
        post_json_with_cookies("/api/matches", &submit_body(mine, missing), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Blue warrior not found");
}

#[sqlx::test]
async fn submit_unparseable_source_is_400_and_not_persisted(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let bad = insert_warrior(&pool, alice, "INVALID NONSENSE").await;
    let good = insert_warrior(&pool, alice, DWARF).await;

    let (status, json) = send(
        router.clone(),
        post_json_with_cookies("/api/matches", &submit_body(bad, good), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .starts_with("Red warrior parse error"));

    let (status, json) = send(
        router,
        post_json_with_cookies("/api/matches", &submit_body(good, bad), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .starts_with("Blue warrior parse error"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn submit_malformed_body_is_rejected(pool: PgPool) {
    let router = app(pool);
    let (_, cookies) = register(&router, "alice").await;

    // Missing field / non-UUID: axum's Json extractor rejects with 422.
    for body in [
        json!({"red_warrior_id": Uuid::new_v4()}),
        json!({"red_warrior_id": "nope", "blue_warrior_id": "nope"}),
    ] {
        let (status, _) = send(
            router.clone(),
            post_json_with_cookies("/api/matches", &body, &cookies),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

// =============================================================================
// Get
// =============================================================================

#[sqlx::test]
async fn get_match_by_id_and_404(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let w = insert_warrior(&pool, alice, DWARF).await;
    let id = insert_match(&pool, (w, alice), (w, alice), 0).await;

    let (status, json) = send(
        router.clone(),
        get_with_cookies(&format!("/api/matches/{id}"), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), MATCH_KEYS);
    assert_eq!(json["id"], id.to_string());
    assert_eq!(json["result"], "tie");
    assert_eq!(json["core_size"], 8000); // column default
    assert_eq!(json["max_steps"], 80000); // column default

    let (status, json) = send(
        router.clone(),
        get_with_cookies(&format!("/api/matches/{}", Uuid::new_v4()), &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Match not found");

    let (status, _) = send(
        router,
        get_with_cookies("/api/matches/not-a-uuid", &cookies),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn get_match_is_participant_only(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, alice_cookies) = register(&router, "alice").await;
    let (bob, bob_cookies) = register(&router, "bob").await;
    let (_, carol_cookies) = register(&router, "carol").await;
    let wa = insert_warrior(&pool, alice, DWARF).await;
    let wb = insert_warrior(&pool, bob, DWARF).await;
    let id = insert_match(&pool, (wa, alice), (wb, bob), 0).await;

    // Both sides can fetch it.
    for cookies in [&alice_cookies, &bob_cookies] {
        let (status, json) = send(
            router.clone(),
            get_with_cookies(&format!("/api/matches/{id}"), cookies),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["id"], id.to_string());
    }

    // A non-participant gets exactly the response a nonexistent id gets.
    let (status, hidden) = send(
        router.clone(),
        get_with_cookies(&format!("/api/matches/{id}"), &carol_cookies),
    )
    .await;
    let (missing_status, missing) = send(
        router,
        get_with_cookies(&format!("/api/matches/{}", Uuid::new_v4()), &carol_cookies),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!((status, &hidden), (missing_status, &missing));
}

// =============================================================================
// List
// =============================================================================

#[sqlx::test]
async fn list_empty_uses_default_paging(pool: PgPool) {
    let router = app(pool);
    let (_, cookies) = register(&router, "alice").await;

    let (status, json) = send(router, get_with_cookies("/api/matches", &cookies)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), ["matches", "page", "per_page", "total"]);
    assert_eq!(json["matches"], json!([]));
    assert_eq!(json["total"], 0);
    assert_eq!(json["page"], 1);
    assert_eq!(json["per_page"], 50);
}

#[sqlx::test]
async fn list_includes_both_sides_newest_first_and_excludes_others(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let (bob, _) = register(&router, "bob").await;
    let (carol, _) = register(&router, "carol").await;
    let wa = insert_warrior(&pool, alice, DWARF).await;
    let wb = insert_warrior(&pool, bob, DWARF).await;
    let wc = insert_warrior(&pool, carol, DWARF).await;

    let as_red = insert_match(&pool, (wa, alice), (wb, bob), 30).await;
    let as_blue = insert_match(&pool, (wb, bob), (wa, alice), 10).await;
    let _others = insert_match(&pool, (wb, bob), (wc, carol), 5).await;
    let mirror = insert_match(&pool, (wa, alice), (wa, alice), 20).await;

    let (status, json) = send(router, get_with_cookies("/api/matches", &cookies)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["total"], 3);
    assert_eq!(ids(&json), [as_blue, mirror, as_red].map(|u| u.to_string()));
    assert_eq!(keys(&json["matches"][0]), MATCH_KEYS);
}

#[sqlx::test]
async fn list_pagination_offsets_and_clamping(pool: PgPool) {
    let router = app(pool.clone());
    let (alice, cookies) = register(&router, "alice").await;
    let w = insert_warrior(&pool, alice, DWARF).await;
    // m[0] newest ... m[4] oldest
    let mut m = Vec::new();
    for i in 0..5 {
        m.push(
            insert_match(&pool, (w, alice), (w, alice), i * 10)
                .await
                .to_string(),
        );
    }

    let get = |q: &str| {
        let router = router.clone();
        let req = get_with_cookies(&format!("/api/matches{q}"), &cookies);
        async move { send(router, req).await }
    };

    let (_, json) = get("?page=2&per_page=2").await;
    assert_eq!(ids(&json), m[2..4]);
    assert_eq!(
        (json["page"].as_i64(), json["per_page"].as_i64()),
        (Some(2), Some(2))
    );
    assert_eq!(json["total"], 5);

    let (_, json) = get("?page=3&per_page=2").await;
    assert_eq!(ids(&json), m[4..5]);

    // Past the end: empty page, total still reported.
    let (status, json) = get("?page=9&per_page=2").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids(&json).is_empty());
    assert_eq!(json["total"], 5);
    assert_eq!(json["page"], 9);

    // page < 1 clamps to 1.
    for q in ["?page=0&per_page=2", "?page=-4&per_page=2"] {
        let (_, json) = get(q).await;
        assert_eq!(json["page"], 1);
        assert_eq!(ids(&json), m[0..2]);
    }

    // per_page clamps to [1, 100].
    let (_, json) = get("?per_page=0").await;
    assert_eq!(json["per_page"], 1);
    assert_eq!(ids(&json), m[0..1]);
    let (_, json) = get("?per_page=-7").await;
    assert_eq!(json["per_page"], 1);
    let (_, json) = get("?per_page=1000").await;
    assert_eq!(json["per_page"], 100);
    assert_eq!(ids(&json).len(), 5);

    // Non-numeric query params are rejected by the Query extractor.
    let (status, _) = get("?page=abc").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
