use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    info!("connected to postgres");

    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("migrations applied");

    Ok(pool)
}
