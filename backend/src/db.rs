use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

/// Connect the backend's Postgres pool.
///
/// Migrations are **not** run here. As of #77 they are applied by an explicit
/// deploy step (the `core-war-migrate` one-shot) before the backend starts, so
/// a broken migration fails the migrations step rather than the backend's
/// health check. Tests migrate their own per-test database via `#[sqlx::test]`.
pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    info!("connected to postgres");

    Ok(pool)
}
