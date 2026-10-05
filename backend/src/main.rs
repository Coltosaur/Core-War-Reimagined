use core_war_backend::{app, auth, config::Config, db, AppConfig, AppState};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core_war_backend=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let pool = db::init_pool(&config.database_url).await?;

    auth::password::warm_dummy_hash();

    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("connected to redis");

    let state = AppState {
        db: pool,
        config: AppConfig {
            frontend_url: config.frontend_url,
            jwt_secret: config.jwt_secret,
            trusted_proxies: config.trusted_proxies,
        },
    };

    let app = app::router(state, redis_conn)?;

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    app::serve(listener, app).await?;

    Ok(())
}
