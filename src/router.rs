use crate::{
    constants::{COOKIE as COOKIE_NAME, USER_ID},
    room::{Room, Rooms},
    templ::{get_template, INDEX_PAGE, ROOM_PAGE},
    user::{User, Users},
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

const TIMEOUT_SECS: usize = 10;

pub type WsEndpoint = Cow<'static, str>;

#[derive(Clone)]
pub struct AppState {
    pub rooms: Rooms,
    pub users: Users,
    pub store: MemoryStore,
    pub ws_endpoint: WsEndpoint,
}
impl FromRef<AppState> for Rooms {
    fn from_ref(app_state: &AppState) -> Rooms {
        app_state.rooms.clone()
    }
}
impl FromRef<AppState> for Users {
    fn from_ref(app_state: &AppState) -> Users {
        app_state.users.clone()
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
    rooms: Rooms,
    users: Users,
    store: MemoryStore,
) -> Router {
    let serve_dir = ServeDir::new("assets");
    let state = AppState {
        rooms,
        users,
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
    let rooms_guard = rooms.read().await;
    let mut room_ids: Vec<Uuid> = Vec::with_capacity(rooms_guard.len());
    for room in rooms_guard.iter() {
        room_ids.push(room.read().await.id);
    }

    let templ = get_template(INDEX_PAGE, context! {rooms => room_ids}).map_err(service_error)?;
    Ok(Html::from(templ))
}

async fn create_room(
    State(rooms): State<Rooms>,
    State(users): State<Users>,
) -> axum::response::Result<impl IntoResponse> {
    let room = Room::new(users).await;
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
    State(ws_endpoint): State<WsEndpoint>,
    State(rooms): State<Rooms>,
    user: User,
) -> axum::response::Result<impl IntoResponse> {
    tracing::debug!("get room id {id}");
    let rooms_guard = rooms.read().await;
    for r in rooms_guard.iter() {
        let room = r.read().await;
        if room.id == id {
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
    }
    Err(ErrorResponse::from(StatusCode::NOT_FOUND))
}

pub async fn build_guest_session_if_none<B>(
    State(store): State<MemoryStore>,
    State(users): State<Users>,
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
            .name(format!("Guest{id}"));
        let mut users_guard = users.write().await;
        users_guard.push(user);
    }

    // do something with `response`...

    Ok(response)
}
