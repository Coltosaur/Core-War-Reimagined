//! Integration tests for the matchmaking pipeline: covers the "battle runs,
//! match row inserts, both users' ratings update — all atomically" contract
//! that regressed as #87 (loop overrun) and #89 (rating updates never called).
//!
//! Each scenario walks the full flow through the extracted
//! [`execute_and_persist_match`] against a real Postgres via `sqlx::test`.
//! Fixtures (users + warriors) are seeded with plain SQL to keep the tests
//! focused on the pipeline itself — the HTTP + Socket.IO layers already have
//! their own coverage.

use core_war_backend::matchmaking::runner::{
    execute_and_persist_match, MatchExecuteError, MatchOutcome,
};
use sqlx::PgPool;
use uuid::Uuid;

const IMP: &str = "MOV.I $0, $1";
const DWARF: &str = include_str!("../../engine/tests/warriors/dwarf.red");
// Mice-Lite is the shortest self-terminating curated warrior in the
// preset library — it copies an imp template a few times, then dies in its
// own landing DAT. Paired with Dwarf it produces a reliably decisive match
// (Blue is dead after ~14 alternating steps → Victory{Red}) without
// depending on any particular attacker beating any particular defender
// within MAX_STEPS. Dwarf-vs-Imp at 8000/80k is actually a tie, so it can't
// stand in for a decisive matchup.
const MICE_LITE: &str = include_str!("../../engine/tests/warriors/mice_lite.red");

const CORE_SIZE: usize = 8000;
const MAX_STEPS: u64 = 80_000;

// K_FACTOR is duplicated here (from backend/src/leaderboard/elo.rs) as a
// deliberate test-side re-declaration: the expected-delta helper below is our
// oracle for what the Elo math should produce, and mirroring the K factor
// keeps the oracle self-contained. If the K factor changes, this test module
// must be updated too — that's the intended coupling.
const K_FACTOR: f64 = 32.0;

fn expected_win_delta(winner: i32, loser: i32) -> i32 {
    let expected = 1.0 / (1.0 + 10.0_f64.powf((loser - winner) as f64 / 400.0));
    (K_FACTOR * (1.0 - expected)).round() as i32
}

fn expected_tie_delta_a(rating_a: i32, rating_b: i32) -> i32 {
    let expected = 1.0 / (1.0 + 10.0_f64.powf((rating_b - rating_a) as f64 / 400.0));
    (K_FACTOR * (0.5 - expected)).round() as i32
}

struct Seeded {
    red_user: Uuid,
    blue_user: Uuid,
    red_warrior: Uuid,
    blue_warrior: Uuid,
}

async fn seed_users_and_warriors(pool: &PgPool, red_source: &str, blue_source: &str) -> Seeded {
    let red_user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("red_player")
    .bind("red@example.com")
    .bind("$argon2id$v=19$m=19456,t=2,p=1$fake$fake")
    .fetch_one(pool)
    .await
    .expect("insert red user");

    let blue_user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind("blue_player")
    .bind("blue@example.com")
    .bind("$argon2id$v=19$m=19456,t=2,p=1$fake$fake")
    .fetch_one(pool)
    .await
    .expect("insert blue user");

    let red_warrior: Uuid = sqlx::query_scalar(
        "INSERT INTO warriors (user_id, name, source) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(red_user)
    .bind("Red Warrior")
    .bind(red_source)
    .fetch_one(pool)
    .await
    .expect("insert red warrior");

    let blue_warrior: Uuid = sqlx::query_scalar(
        "INSERT INTO warriors (user_id, name, source) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(blue_user)
    .bind("Blue Warrior")
    .bind(blue_source)
    .fetch_one(pool)
    .await
    .expect("insert blue warrior");

    Seeded {
        red_user,
        blue_user,
        red_warrior,
        blue_warrior,
    }
}

async fn set_rating(pool: &PgPool, user_id: Uuid, rating: i32) {
    sqlx::query("UPDATE users SET rating = $1 WHERE id = $2")
        .bind(rating)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed rating");
}

async fn current_rating(pool: &PgPool, user_id: Uuid) -> i32 {
    sqlx::query_scalar("SELECT rating FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read rating")
}

async fn match_row_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM matches")
        .fetch_one(pool)
        .await
        .expect("count matches")
}

async fn run(pool: &PgPool, s: &Seeded, red_src: &str, blue_src: &str) -> MatchOutcome {
    execute_and_persist_match(
        pool,
        s.red_user,
        s.blue_user,
        s.red_warrior,
        s.blue_warrior,
        red_src,
        blue_src,
        CORE_SIZE,
        MAX_STEPS,
    )
    .await
    .expect("execute_and_persist_match")
}

// =============================================================================
// Scenario 1 — decisive match, red wins early
// =============================================================================

#[sqlx::test]
async fn decisive_match_red_win_updates_ratings(pool: PgPool) {
    // Dwarf-vs-mice-lite (rather than Dwarf-vs-Imp) because Dwarf-vs-Imp at the
    // production 8000/80k parameters is actually a tie — Imp is fast enough
    // that Dwarf's ~13k bombs across an 80k-step match can miss it entirely.
    // Mice-Lite replicates for a handful of iterations, then falls through
    // to its own DAT landing pad and dies — a reliably decisive match.
    let s = seed_users_and_warriors(&pool, DWARF, MICE_LITE).await;
    let outcome = run(&pool, &s, DWARF, MICE_LITE).await;

    assert_eq!(outcome.result, "red_win");
    // Issue #87: loop must stop the moment the decision is made, not pin
    // steps_taken at MAX_STEPS. Mice-Lite self-terminates in a handful of
    // its own turns; with round-robin alternation that's well under 50 total
    // steps — far below MAX_STEPS.
    assert!(
        outcome.steps_taken < 50,
        "mice-lite self-terminates quickly; got {}",
        outcome.steps_taken
    );

    // Both players started at the 1200 default.
    assert_eq!(outcome.red_rating_before, 1200);
    assert_eq!(outcome.blue_rating_before, 1200);

    // Zero-sum against equal ratings.
    let delta = expected_win_delta(1200, 1200);
    assert_eq!(outcome.red_rating_after, 1200 + delta);
    assert_eq!(outcome.blue_rating_after, 1200 - delta);
    assert_eq!(
        outcome.red_rating_after + outcome.blue_rating_after,
        1200 + 1200,
        "zero-sum invariant"
    );

    // The persisted state agrees with the returned outcome.
    assert_eq!(
        current_rating(&pool, s.red_user).await,
        outcome.red_rating_after
    );
    assert_eq!(
        current_rating(&pool, s.blue_user).await,
        outcome.blue_rating_after
    );
    assert_eq!(match_row_count(&pool).await, 1);
}

// =============================================================================
// Scenario 2 — decisive match, blue wins early (side-mapping regression guard)
// =============================================================================

#[sqlx::test]
async fn decisive_match_blue_win_updates_ratings(pool: PgPool) {
    // Same warriors, swapped sides. This is the case a broken winner/loser
    // remap tuple would silently botch — the winner gets rating-lowering
    // updates and the loser gets a boost. Assert the direction explicitly.
    let s = seed_users_and_warriors(&pool, MICE_LITE, DWARF).await;
    let outcome = run(&pool, &s, MICE_LITE, DWARF).await;

    assert_eq!(outcome.result, "blue_win");
    assert!(
        outcome.steps_taken < 50,
        "mice-lite self-terminates quickly; got {}",
        outcome.steps_taken
    );

    let delta = expected_win_delta(1200, 1200);
    assert_eq!(outcome.blue_rating_after, 1200 + delta);
    assert_eq!(outcome.red_rating_after, 1200 - delta);
    assert!(outcome.blue_rating_after > outcome.red_rating_after);

    assert_eq!(
        current_rating(&pool, s.red_user).await,
        outcome.red_rating_after
    );
    assert_eq!(
        current_rating(&pool, s.blue_user).await,
        outcome.blue_rating_after
    );
}

// =============================================================================
// Scenario 3 — genuine tie at equal ratings
// =============================================================================

#[sqlx::test]
async fn tie_at_equal_ratings_no_change(pool: PgPool) {
    // Imp-vs-Imp is the canonical "runs to MAX_STEPS and ties" match — the
    // case that distinguishes #87 ("stops too early") from the correct behavior
    // ("only stops early when a Victory is decided").
    let s = seed_users_and_warriors(&pool, IMP, IMP).await;
    let outcome = run(&pool, &s, IMP, IMP).await;

    assert_eq!(outcome.result, "tie");
    assert_eq!(
        outcome.steps_taken as u64, MAX_STEPS,
        "a genuine tie should still run to MAX_STEPS"
    );

    // tie_change of equal ratings is a no-op.
    assert_eq!(outcome.red_rating_after, 1200);
    assert_eq!(outcome.blue_rating_after, 1200);
    assert_eq!(current_rating(&pool, s.red_user).await, 1200);
    assert_eq!(current_rating(&pool, s.blue_user).await, 1200);
}

// =============================================================================
// Scenario 4 — tie at unequal ratings shifts toward the lower-rated player
// =============================================================================

#[sqlx::test]
async fn tie_at_unequal_ratings_shifts_toward_lower(pool: PgPool) {
    let s = seed_users_and_warriors(&pool, IMP, IMP).await;
    set_rating(&pool, s.red_user, 1600).await;
    set_rating(&pool, s.blue_user, 1200).await;

    let outcome = run(&pool, &s, IMP, IMP).await;

    assert_eq!(outcome.result, "tie");
    assert_eq!(outcome.red_rating_before, 1600);
    assert_eq!(outcome.blue_rating_before, 1200);

    let delta_red = expected_tie_delta_a(1600, 1200);
    assert!(
        delta_red < 0,
        "higher-rated player should lose points on a tie"
    );
    assert_eq!(outcome.red_rating_after, 1600 + delta_red);
    assert_eq!(outcome.blue_rating_after, 1200 - delta_red);
    assert!(outcome.red_rating_after < 1600);
    assert!(outcome.blue_rating_after > 1200);
    assert_eq!(
        outcome.red_rating_after + outcome.blue_rating_after,
        1600 + 1200,
        "zero-sum invariant"
    );
}

// =============================================================================
// Scenario 5 — errored match (parse failure) leaves ratings and matches table alone
// =============================================================================

#[sqlx::test]
async fn parse_error_no_side_effects(pool: PgPool) {
    let s = seed_users_and_warriors(&pool, "TOTAL NONSENSE 1234", IMP).await;

    let result = execute_and_persist_match(
        &pool,
        s.red_user,
        s.blue_user,
        s.red_warrior,
        s.blue_warrior,
        "TOTAL NONSENSE 1234",
        IMP,
        CORE_SIZE,
        MAX_STEPS,
    )
    .await;

    match result {
        Err(MatchExecuteError::ParseRed(_)) => {}
        Err(e) => panic!("expected ParseRed, got {e:?}"),
        Ok(o) => panic!("expected parse error, got Ok({o:?})"),
    }

    assert_eq!(
        match_row_count(&pool).await,
        0,
        "no match row on parse error"
    );
    assert_eq!(current_rating(&pool, s.red_user).await, 1200);
    assert_eq!(current_rating(&pool, s.blue_user).await, 1200);
}

// =============================================================================
// Atomicity invariant — either both (match row inserted, ratings updated) or
// neither. Cross-checks against every scenario above: after a successful call
// the ratings AND the match count MUST agree, and after a failure BOTH must
// be untouched. This is what a broken `execute_and_persist_match` that
// commits the match row before updating ratings would fail.
// =============================================================================

#[sqlx::test]
async fn atomicity_on_success(pool: PgPool) {
    let s = seed_users_and_warriors(&pool, DWARF, MICE_LITE).await;
    let outcome = run(&pool, &s, DWARF, MICE_LITE).await;

    // Both writes present.
    assert_eq!(match_row_count(&pool).await, 1);
    assert_ne!(current_rating(&pool, s.red_user).await, 1200);
    assert_ne!(current_rating(&pool, s.blue_user).await, 1200);

    // And they agree.
    assert_eq!(
        current_rating(&pool, s.red_user).await,
        outcome.red_rating_after
    );
    assert_eq!(
        current_rating(&pool, s.blue_user).await,
        outcome.blue_rating_after
    );
}

// Note on the `all_dead` outcome: with the #87 short-circuit fix in place,
// AllDead is effectively unreachable in a two-warrior match. `step()` is
// round-robin per warrior, so if warrior A dies on turn N the runner breaks
// at that step (result() reports Victory{B}) before B could also die on
// turn N+1. The MatchExecuteError code path still maps AllDead → tie_change
// as a correctness safety net (documented in runner.rs), and if the engine
// ever grows a simultaneous-death primitive this file should grow a scenario
// asserting the tie-rating semantics for it.
