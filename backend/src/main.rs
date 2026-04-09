use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use socketioxide::{extract::SocketRef, SocketIo};
use std::{env, net::SocketAddr};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

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

    let frontend_url =
        env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    let (socket_layer, io) = SocketIo::new_layer();
    io.ns("/", on_connect);

    let cors = CorsLayer::new()
        .allow_origin(frontend_url.parse::<axum::http::HeaderValue>()?)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .layer(socket_layer)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
