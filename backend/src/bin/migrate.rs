//! One-shot database migrator for the explicit deploy step (issue #77).
//!
//! Production runs migrations here, before the backend container starts,
//! instead of at backend startup. `db::init_pool` no longer migrates. The
//! migration set is embedded at compile time by `sqlx::migrate!`, so this
//! binary and the backend ship in the same image and apply the exact
//! migrations of the release they were built from.
//!
//! The test suite still migrates its own per-test database (`#[sqlx::test]`),
//! so this binary is not involved there.

use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url =
        env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set (check the deploy env)")?;

    // One connection is plenty for a one-shot migrator, and it keeps the
    // footprint small when this runs as a short-lived `compose run` container.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    println!("migrations applied");

    Ok(())
}
