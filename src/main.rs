#![allow(dead_code, unused_variables)]
mod constants;
mod room;
mod router;
mod templ;
use std::{
    env::var,
    error::Error,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use constants::{SERVICE_APPLICATION_NAME, SERVICE_HOST, SERVICE_PORT};
use router::{get_router, setup_tracing};
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_tracing()?;

    let host = var(SERVICE_HOST).unwrap_or_else(|_| String::from("127.0.0.1"));
    let port = var(SERVICE_PORT).unwrap_or_else(|_| String::from("8080"));
    let app_name = var(SERVICE_APPLICATION_NAME).unwrap_or_else(|_| String::from("heartz"));
    let addr = SocketAddr::from_str(&format!("{host}:{port}"))?;
    let rooms = Arc::new(Mutex::new(Vec::with_capacity(100)));
    let app = get_router(rooms.clone());

    tracing::info!("{app_name} :: listening on {:?}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
