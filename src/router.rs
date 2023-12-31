use crate::data::{Room, Rooms, User};
use crate::{
    constants::{COOKIE as COOKIE_NAME, TIMEOUT_SECS, USER_ID},
    db::upsert_user,
    templ::{get_template, INDEX_PAGE, ROOM_PAGE},
    utils::service_error,
    websocket::ws_handler,
};
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    extract::{FromRef, Path, State},
    http::{header::SET_COOKIE, Request, StatusCode},
    middleware::Next,
    response::{ErrorResponse, Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::CookieJar;
use chrono::Local;
use minijinja::context;
use sqlx::{Pool, Sqlite};
use std::{borrow::Cow, error::Error};
use time::{macros::format_description, UtcOffset};
use tower::ServiceBuilder;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::Level;
use tracing_subscriber::{fmt::time::OffsetTime, EnvFilter, FmtSubscriber};
use uuid::Uuid;

pub type WsEndpoint = Cow<'static, str>;

#[derive(Clone)]
pub struct AppState {
    pub rooms: Rooms,
    pub db_pool: Pool<Sqlite>,
    pub store: MemoryStore,
    pub ws_endpoint: WsEndpoint,
}
impl FromRef<AppState> for Rooms {
    fn from_ref(app_state: &AppState) -> Rooms {
        app_state.rooms.clone()
    }
}
impl FromRef<AppState> for Pool<Sqlite> {
    fn from_ref(app_state: &AppState) -> Pool<Sqlite> {
        app_state.db_pool.clone()
    }
}
impl FromRef<AppState> for WsEndpoint {
    fn from_ref(app_state: &AppState) -> WsEndpoint {
        app_state.ws_endpoint.clone()
    }
}
impl FromRef<AppState> for MemoryStore {
    fn from_ref(app_state: &AppState) -> MemoryStore {
        app_state.store.clone()
    }
}
pub fn get_router(
    ws_endpoint: Cow<'static, str>,
    db_pool: Pool<Sqlite>,
    rooms: Rooms,
    store: MemoryStore,
) -> Router {
    let serve_dir = ServeDir::new("assets");
    let state = AppState {
        rooms,
        db_pool,
        store,
        ws_endpoint,
    };
    Router::new()
        .route("/create-room", post(create_room))
        .route("/room/:id", get(get_room))
        .route("/ws/:id", get(ws_handler))
        .route("/", get(index_page))
        .nest_service("/assets", serve_dir)
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/assets/icon/favicon.ico") }),
        )
        .route_layer(
            ServiceBuilder::new().layer(axum::middleware::from_fn_with_state(
                state.clone(),
                build_guest_session_if_none,
            )),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .with_state(state)
}
pub fn setup_tracing() -> Result<(), Box<dyn Error>> {
    let offset_hours = {
        let now = Local::now();
        let offset_seconds = now.offset().local_minus_utc();
        let hours = offset_seconds / 3600;
        hours as i8
    };
    let offset = UtcOffset::from_hms(offset_hours, 0, 0)?;

    let timer = OffsetTime::new(
        offset,
        format_description!("[day]-[month]-[year] [hour]:[minute]:[second]"),
    );
    let subscriber = FmtSubscriber::builder()
        .with_timer(timer)
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

async fn index_page(State(rooms): State<Rooms>) -> axum::response::Result<impl IntoResponse> {
    let templ = get_template(
        INDEX_PAGE,
        context! {rooms => rooms.iter().map(|e|*e.key()).collect::<Vec<_>>()},
    )
    .map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(
    State(rooms): State<Rooms>,
    State(pool): State<Pool<Sqlite>>,
) -> axum::response::Result<impl IntoResponse> {
    let (id, room) = Room::new(pool).await;
    let response = Redirect::to(&format!("/room/{}", id));

    rooms.insert(id, room);

    Ok(response)
}

async fn get_room(
    Path(id): Path<Uuid>,
    State(ws_endpoint): State<WsEndpoint>,
    State(rooms): State<Rooms>,
    user: User,
) -> axum::response::Result<impl IntoResponse> {
    tracing::debug!("get room id {id}");
    if let Some(room) = rooms.get(&id) {
        let room = room.read().await;
        let templ = get_template(
            ROOM_PAGE,
            context!(
                room => *room,
                ws_endpoint => ws_endpoint,
                user => user,
                timeout => TIMEOUT_SECS
            ),
        )
        .map_err(service_error)?;
        return Ok(Html::from(templ));
    }

    Err(ErrorResponse::from(StatusCode::NOT_FOUND))
}

pub async fn build_guest_session_if_none<B>(
    State(store): State<MemoryStore>,
    State(pool): State<Pool<Sqlite>>,
    request: Request<B>,
    next: Next<B>,
) -> axum::response::Result<impl IntoResponse> {
    let cookies = CookieJar::from_headers(request.headers());
    let mut response = next.run(request).await;
    if cookies.get(COOKIE_NAME).is_none() {
        tracing::debug!("session doesn't exist, create one");
        let id = Uuid::new_v4();
        let mut session = Session::new();
        session.insert(USER_ID, id).map_err(service_error)?;
        // Store session and get corresponding cookie
        let cookie = store.store_session(session).await.map_err(service_error)?;
        tracing::debug!("{cookie:?}");
        let cookie = cookie.ok_or_else(|| service_error("failed  to store session"))?;
        // Build the cookie
        let cookie = format!("{}={}; SameSite=Lax; Path=/", COOKIE_NAME, cookie);
        // Set cookie
        response
            .headers_mut()
            .insert(SET_COOKIE, cookie.parse().map_err(service_error)?);
        let user = User::default()
            .with_id(id)
            .human(true)
            .is_guest(true)
            .name(format!("Guest{id}"));
        upsert_user(user, &pool)
            .await
            .map_err(|e| service_error(format!("couldn't save user {user:?} => {e}")))?;
    }

    // do something with `response`...

    Ok(response)
}
