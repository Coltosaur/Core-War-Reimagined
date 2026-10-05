//! Serves the production app over a real socket. Other integration tests
//! build their own routers and call them with `oneshot`, which skips the
//! server entirely, so they can't see how `main` starts it. The server was
//! once started without client addresses, which put every visitor in one
//! login rate-limit bucket: five logins from anyone locked everyone out (#116).

use core_war_backend::{app, AppConfig, AppState};
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const FRONTEND_URL: &str = "http://localhost:5173";

async fn production_router(pool: PgPool) -> axum::Router {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let redis = ConnectionManager::new(redis::Client::open(url.as_str()).unwrap())
        .await
        .expect("connect to redis");
    let state = AppState {
        db: pool,
        config: AppConfig {
            frontend_url: FRONTEND_URL.into(),
            jwt_secret: b"integration-test-secret-that-is-at-least-32-bytes!!".to_vec(),
            trusted_proxies: vec![],
        },
    };
    app::router(state, redis).expect("build production router")
}

/// POST a login for an account that doesn't exist and return the status.
async fn failed_login_status(addr: SocketAddr) -> u16 {
    let body = r#"{"username_or_email":"no_such_user_116","password":"wrong-password"}"#;
    let request = format!(
        "POST /api/auth/login HTTP/1.1\r\nHost: {addr}\r\nOrigin: {FRONTEND_URL}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("unparseable response: {response:?}"))
}

#[sqlx::test]
async fn production_server_gives_the_rate_limiter_client_addresses(pool: PgPool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(app::serve(listener, production_router(pool).await));

    let status = failed_login_status(addr).await;
    // 401 is the handler rejecting the credentials. 429 is also acceptable:
    // this shares 127.0.0.1's login bucket with whatever else ran locally,
    // and either way the limiter got an address. 500 is the failure: the
    // limiter had no client address.
    assert!(
        status == 401 || status == 429,
        "expected 401 (or 429 from a busy local bucket), got {status}"
    );
}

#[sqlx::test]
async fn serving_without_client_addresses_fails_closed(pool: PgPool) {
    // What `main` used to do. Pins that the limiter refuses rather than
    // falling back to one shared bucket, which is what lets the test above
    // detect the regression.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = production_router(pool).await;
    tokio::spawn(async move { axum::serve(listener, router).await });

    assert_eq!(failed_login_status(addr).await, 500);
}
