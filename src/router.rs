use async_session::SessionStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{ErrorResponse, Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use minijinja::context;
use std::{error::Error, sync::Arc};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{
    room::{room_task, Room},
    templ::{get_template, INDEX_PAGE, ROOM_PAGE},
    utils::service_error,
};
pub type Rooms = Vec<Arc<RwLock<Room>>>;

pub fn get_router(rooms: Rooms, store: impl SessionStore) -> Router {
    let serve_dir = ServeDir::new("assets");
    Router::new()
        .route("/create-room", post(create_room))
        .route("/room/:id", get(get_room))
        .route("/", get(index_page))
        .nest_service("/assets", serve_dir)
        .with_state(rooms)
        .with_state(store)
}
pub fn setup_tracing() -> Result<(), Box<dyn Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

async fn index_page(State(rooms): State<Rooms>) -> axum::response::Result<impl IntoResponse> {
    let mut room_ids: Vec<String> = Vec::with_capacity(rooms.len());
    for room in rooms.iter() {
        room_ids.push(room.read().await.id.clone());
    }

    let templ = get_template(INDEX_PAGE, context! {rooms => room_ids}).map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(State(mut rooms): State<Rooms>) -> axum::response::Result<impl IntoResponse> {
    let room = Room::default();
    let response = Redirect::to(&format!("/room/{}", room.id));
    let room = Arc::new(RwLock::new(room));

    let (clone, room) = (room.clone(), room);
    rooms.push(room);

    let task = tokio::spawn(room_task(clone));
    // todo i stopped there

    Ok(response)
}

async fn get_room(
    Path(id): Path<String>,
    State(rooms): State<Rooms>,
) -> axum::response::Result<impl IntoResponse> {
    for r in rooms.iter() {
        let room = r.read().await;
        if room.id == id {
            let templ = get_template(
                ROOM_PAGE,
                context!(
                    room => *room
                ),
            )
            .map_err(service_error)?;
            return Ok(Html::from(templ));
        }
    }
    Err(ErrorResponse::from(StatusCode::NOT_FOUND))
}
