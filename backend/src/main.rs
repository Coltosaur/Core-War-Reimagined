use axum::extract::DefaultBodyLimit;
use axum::http::{header, Method};
use axum::{middleware, routing::get, routing::post, Router};
use core_war_backend::{
    auth, config::Config, db, health, leaderboard, matches, matchmaking, profile, warriors,
    AppConfig, AppState,
};

const MAX_REQUEST_BODY_BYTES: usize = 128 * 1024;
use socketioxide::{extract::SocketRef, SocketIo};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
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
            frontend_url: config.frontend_url.clone(),
            jwt_secret: config.jwt_secret,
            trusted_proxies: config.trusted_proxies,
        },
    };

    let jwt_secret_for_socket = state.config.jwt_secret.clone();
    let queue = matchmaking::queue::RedisQueue::new(redis_conn.clone());
    let db_for_socket = state.db.clone();
    let (socket_layer, io) = SocketIo::new_layer();
    let io_for_handler = io.clone();
    io.ns("/", move |socket: SocketRef| {
        auth::socket::on_connect(socket.clone(), jwt_secret_for_socket.clone());
        matchmaking::events::register_events(
            &socket,
            io_for_handler.clone(),
            queue.clone(),
            db_for_socket.clone(),
        );
    });

    let cors = CorsLayer::new()
        .allow_origin(config.frontend_url.parse::<axum::http::HeaderValue>()?)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true);

    let proxies = state.config.trusted_proxies.clone();
    let login_limiter = auth::rate_limit::login_limiter(redis_conn.clone(), proxies.clone());
    let register_limiter = auth::rate_limit::register_limiter(redis_conn.clone(), proxies.clone());
    let refresh_limiter = auth::rate_limit::refresh_limiter(redis_conn, proxies);

    let app = Router::new()
        .route("/health", get(health::liveness))
        .route("/health/deep", get(health::readiness))
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
        .route("/api/auth/me", get(auth::handlers::me))
        .route(
            "/api/warriors",
            get(warriors::handlers::list).post(warriors::handlers::create),
        )
        .route(
            "/api/warriors/:id",
            get(warriors::handlers::get)
                .put(warriors::handlers::update)
                .delete(warriors::handlers::delete),
        )
        .route("/api/leaderboard", get(leaderboard::handlers::leaderboard))
        .route(
            "/api/matches",
            get(matches::handlers::list).post(matches::handlers::submit),
        )
        .route("/api/matches/:id", get(matches::handlers::get))
        .route("/api/profile", get(profile::handlers::me))
        .route(
            "/api/users/:username",
            get(profile::handlers::public_profile),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::csrf_middleware,
        ))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
        .layer(socket_layer)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
