use std::{
    error::Error,
    fmt::Display,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use minijinja::context;
use tower_http::services::{ServeDir, ServeFile};
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{
    room::Room,
    templ::{get_template, INDEX_PAGE},
};

pub fn get_router(rooms: Arc<Mutex<Vec<Room>>>) -> Router {
    let serve_dir = ServeDir::new("assets");
    Router::new()
        .route("/create-room", post(create_room))
        .route("/room/:id", get(get_room))
        .nest_service("/", get(index_page))
        .nest_service("/assets", serve_dir)
        .with_state(rooms)
}
pub fn setup_tracing() -> Result<(), Box<dyn Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

async fn index_page() -> axum::response::Result<impl IntoResponse> {
    let templ = get_template(INDEX_PAGE, context! {}).map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(
    State(rooms): State<Arc<Mutex<Vec<Room>>>>,
) -> axum::response::Result<impl IntoResponse> {
    let room = Room::default();
    let response = Redirect::permanent(&format!("/room/{}", room.id));
    let mut rooms = rooms.lock().map_err(service_error)?;
    rooms.push(room);

    Ok(response)
}

async fn get_room(
    Path(id): Path<String>,
    State(rooms): State<Arc<Mutex<Vec<Room>>>>,
) -> axum::response::Result<impl IntoResponse> {
    let rooms = rooms.lock().map_err(service_error)?;
    let room = rooms
        .iter()
        .find(|r| r.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Html::from(format!("<p> Room {id} </p>")))
}

fn service_error(e: impl Display) -> StatusCode {
    tracing::error!("service error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}
