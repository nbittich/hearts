#![allow(dead_code, unused_variables)]
mod constants;
mod room;
mod router;
mod templ;
mod user;
mod utils;
mod websocket;
use std::{env::var, error::Error, net::SocketAddr, str::FromStr, sync::Arc};

use async_session::MemoryStore;
use constants::{SERVICE_APPLICATION_NAME, SERVICE_HOST, SERVICE_PORT};
use dashmap::{DashMap, DashSet};
use router::{get_router, setup_tracing};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::constants::{SQLITE_DB_URL, WS_ENDPOINT};
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_tracing()?;

    let host = var(SERVICE_HOST).unwrap_or_else(|_| String::from("0.0.0.0"));
    let sqlite_url = var(SQLITE_DB_URL).unwrap_or_else(|_| String::from("sqlite:/tmp/data.db"));
    let ws_endpoint = var(WS_ENDPOINT).unwrap_or_else(|_| String::from("ws://localhost:8080/ws"));
    let port = var(SERVICE_PORT).unwrap_or_else(|_| String::from("8080"));
    let app_name = var(SERVICE_APPLICATION_NAME).unwrap_or_else(|_| String::from("heartz"));
    let addr = SocketAddr::from_str(&format!("{host}:{port}"))?;
    let rooms = Arc::new(DashMap::with_capacity(100));
    let db_pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(SqliteConnectOptions::from_str(&sqlite_url)?.create_if_missing(true))
        .await?;

    let users = Arc::new(DashSet::with_capacity(100));
    let store = MemoryStore::new();
    let app = get_router(
        std::borrow::Cow::Owned(ws_endpoint),
        db_pool,
        rooms,
        users,
        store,
    );

    tracing::info!("{app_name} :: listening on {:?}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}
