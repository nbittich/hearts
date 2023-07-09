use std::{
    error::Error,
    fmt::Display,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use minijinja::context;
use tower_http::services::ServeDir;
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{
    room::Room,
    templ::{get_template, INDEX_PAGE, ROOM_PAGE},
};

pub fn get_router(rooms: Arc<RwLock<Vec<Room>>>) -> Router {
    let serve_dir = ServeDir::new("assets");
    Router::new()
        .route("/create-room", post(create_room))
        .route("/room/:id", get(get_room))
        .route("/", get(index_page))
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

async fn index_page(
    State(rooms): State<Arc<RwLock<Vec<Room>>>>,
) -> axum::response::Result<impl IntoResponse> {
    let rooms = rooms.read().map_err(service_error)?;

    let templ = get_template(INDEX_PAGE, context! {rooms => *rooms}).map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(
    State(rooms): State<Arc<RwLock<Vec<Room>>>>,
) -> axum::response::Result<impl IntoResponse> {
    let room = Room::default();
    let response = Redirect::to(&format!("/room/{}", room.id));
    let mut rooms = rooms.write().map_err(service_error)?;
    rooms.push(room);

    Ok(response)
}

async fn get_room(
    Path(id): Path<String>,
    State(rooms): State<Arc<RwLock<Vec<Room>>>>,
) -> axum::response::Result<impl IntoResponse> {
    let rooms = rooms.read().map_err(service_error)?;
    let room = rooms
        .iter()
        .find(|r| r.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let templ = get_template(
        ROOM_PAGE,
        context!(
            room => *room
        ),
    )
    .map_err(service_error)?;
    Ok(Html::from(templ))
}

fn service_error(e: impl Display) -> StatusCode {
    tracing::error!("service error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}
