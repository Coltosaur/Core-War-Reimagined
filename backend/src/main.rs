use axum::{middleware, routing::get, routing::post, Json, Router};
use serde_json::{json, Value};
use socketioxide::{extract::SocketRef, SocketIo};
use sqlx::PgPool;
use std::net::{IpAddr, SocketAddr};
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
    pub trusted_proxies: Vec<IpAddr>,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
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
            trusted_proxies: config.trusted_proxies,
        },
    };

    let jwt_secret_for_socket = state.config.jwt_secret.clone();
    let (socket_layer, io) = SocketIo::new_layer();
    io.ns("/", move |socket: SocketRef| {
        auth::socket::on_connect(socket, jwt_secret_for_socket.clone());
    });

    let cors = CorsLayer::new()
        .allow_origin(config.frontend_url.parse::<axum::http::HeaderValue>()?)
        .allow_methods(Any)
        .allow_headers(Any);

    let proxies = state.config.trusted_proxies.clone();
    let login_limiter = auth::rate_limit::login_limiter(proxies.clone());
    let register_limiter = auth::rate_limit::register_limiter(proxies.clone());
    let refresh_limiter = auth::rate_limit::refresh_limiter(proxies);

    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/api/auth/register",
            post(auth::handlers::register).layer(middleware::from_fn_with_state(
                register_limiter,
                auth::rate_limit::rate_limit_middleware,
            )),
        )
        .route(
            "/api/auth/login",
            post(auth::handlers::login).layer(middleware::from_fn_with_state(
                login_limiter,
                auth::rate_limit::rate_limit_middleware,
            )),
        )
        .route(
            "/api/auth/refresh",
            post(auth::handlers::refresh).layer(middleware::from_fn_with_state(
                refresh_limiter,
                auth::rate_limit::rate_limit_middleware,
            )),
        )
        .route("/api/auth/logout", post(auth::handlers::logout))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .with_state(state)
        .layer(socket_layer)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
