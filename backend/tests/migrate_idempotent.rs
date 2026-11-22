//! #77: the embedded migrator must be safe to run more than once.
//!
//! In production the `core-war-migrate` one-shot runs on every deploy —
//! including deploys that add no new migration — so applying the set against an
//! already-migrated database must be a no-op, not an error. This test exercises
//! the same embedded `sqlx::migrate!("./migrations")` the binary uses.

use sqlx::PgPool;

// `migrations = false` hands us a freshly created but un-migrated database, so
// the two explicit runs below are the first and second application.
#[sqlx::test(migrations = false)]
async fn embedded_migrator_is_idempotent(pool: PgPool) {
    let migrator = sqlx::migrate!("./migrations");

    migrator
        .run(&pool)
        .await
        .expect("first migrate run should apply cleanly");

    // Second run against the already-migrated schema: sqlx records applied
    // versions in _sqlx_migrations and skips them, so this is a no-op.
    migrator
        .run(&pool)
        .await
        .expect("second migrate run should be a no-op, not an error");
}
