pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
mod models;

use sqlx::PgPool;
use std::net::IpAddr;

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
