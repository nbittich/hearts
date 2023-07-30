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
use uuid::Uuid;

use crate::{
    room::Room,
    templ::{get_template, INDEX_PAGE, ROOM_PAGE},
    utils::service_error,
};
pub type Rooms = Arc<RwLock<Vec<Arc<RwLock<Room>>>>>;

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
    let rooms_guard = rooms.read().await;
    let mut room_ids: Vec<Uuid> = Vec::with_capacity(rooms_guard.len());
    for room in rooms_guard.iter() {
        room_ids.push(room.read().await.id);
    }

    let templ = get_template(INDEX_PAGE, context! {rooms => room_ids}).map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(State(rooms): State<Rooms>) -> axum::response::Result<impl IntoResponse> {
    let room = Room::new().await;
    let (clone, room) = (room.clone(), room);
    let room_guard = clone.read().await;
    let response = Redirect::to(&format!("/room/{}", room_guard.id));

    let mut rooms_guard = rooms.write().await;
    rooms_guard.push(room);

    // todo i stopped there

    Ok(response)
}

async fn get_room(
    Path(id): Path<Uuid>,
    State(rooms): State<Rooms>,
) -> axum::response::Result<impl IntoResponse> {
    tracing::info!("room id {id}");
    let rooms_guard = rooms.read().await;
    for r in rooms_guard.iter() {
        let room = r.read().await;
        if room.id == id {
            tracing::info!("gettin in");
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
