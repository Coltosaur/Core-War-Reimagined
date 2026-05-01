use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use socketioxide::{extract::SocketRef, SocketIo};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

pub mod auth;
mod config;
mod db;
pub mod errors;
mod models;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: AppConfig,
}

#[derive(Clone)]
pub struct AppConfig {
    pub frontend_url: String,
    pub jwt_secret: Vec<u8>,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

fn on_connect(socket: SocketRef) {
    info!("client connected: {}", socket.id);

    socket.on_disconnect(|socket: SocketRef| {
        info!("client disconnected: {}", socket.id);
    });
}

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

    let state = AppState {
        db: pool,
        config: AppConfig {
            frontend_url: config.frontend_url.clone(),
            jwt_secret: config.jwt_secret,
        },
    };

    let (socket_layer, io) = SocketIo::new_layer();
    io.ns("/", on_connect);

    let cors = CorsLayer::new()
        .allow_origin(config.frontend_url.parse::<axum::http::HeaderValue>()?)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .with_state(state)
        .layer(socket_layer)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
