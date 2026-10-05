//! The production HTTP + Socket.IO app, built here rather than in `main` so
//! integration tests can serve exactly what production serves. Tests that
//! hand-roll their own `Router` can't catch wiring bugs — like the server
//! once being started without client addresses, which put every visitor in
//! one rate-limit bucket (#116).

use crate::{
    account, auth, health, leaderboard, matches, matchmaking, profile, warriors, AppState,
};
use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::{middleware, routing::get, routing::post, Router};
use redis::aio::ConnectionManager;
use socketioxide::{extract::SocketRef, SocketIo};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 128 * 1024;

pub fn router(
    state: AppState,
    redis_conn: ConnectionManager,
) -> Result<Router, Box<dyn std::error::Error>> {
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
        .allow_origin(state.config.frontend_url.parse::<HeaderValue>()?)
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
    let login_limiter = auth::failure_limit::login_failure_limiter(
        redis_conn.clone(),
        proxies.clone(),
        state.db.clone(),
    );
    let register_limiter = auth::rate_limit::register_limiter(redis_conn.clone(), proxies.clone());
    let refresh_limiter = auth::rate_limit::refresh_limiter(redis_conn.clone(), proxies.clone());
    let change_password_limiter = auth::failure_limit::change_password_failure_limiter(
        redis_conn.clone(),
        proxies,
        &state.config.jwt_secret,
    );

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
                auth::failure_limit::failure_limit_middleware,
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
            "/api/auth/change-password",
            post(auth::handlers::change_password).layer(middleware::from_fn_with_state(
                change_password_limiter,
                auth::failure_limit::failure_limit_middleware,
            )),
        )
        .route("/api/account", get(account::handlers::me))
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

    Ok(app)
}

/// Serve `app` on `listener`. Always use this rather than `axum::serve`
/// directly: the rate limiters key on each request's peer address, which
/// only exists when the service is built with connect info.
pub async fn serve(listener: TcpListener, app: Router) -> std::io::Result<()> {
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}
