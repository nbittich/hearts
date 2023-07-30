use std::{fmt::Display, sync::atomic::AtomicU64};

use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    extract::State,
    http::{header::SET_COOKIE, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;

use crate::constants::{COOKIE as COOKIE_NAME, USER_ID};

#[derive(Debug)]
pub struct HomePageRedirect;

impl IntoResponse for HomePageRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/").into_response()
    }
}
pub fn service_error(e: impl Display) -> impl IntoResponse {
    tracing::error!("service error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}
static SESSION_ID: AtomicU64 = AtomicU64::new(1);

pub async fn build_guest_session_if_none<B>(
    State(store): State<MemoryStore>,
    request: Request<B>,
    next: Next<B>,
) -> axum::response::Result<impl IntoResponse> {
    let cookies = CookieJar::from_headers(request.headers());
    let mut response = next.run(request).await;
    if cookies.get(COOKIE_NAME).is_none() {
        tracing::debug!("session doesn't exist, create one");
        let id = SESSION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    }

    // do something with `response`...

    Ok(response)
}

#[cfg(test)]
mod test {
    use lib_hearts::PLAYER_NUMBER;
    use uuid::Uuid;

    use crate::{constants::ID_PLAYER_BOT, room::User};

    #[test]
    fn test_to_static_array1() {
        let players: [Option<Uuid>; PLAYER_NUMBER] = [
            Some(Uuid::from_u128(1)),
            Some(Uuid::from_u128(2)),
            Some(Uuid::from_u128(3)),
            Some(Uuid::from_u128(4)),
        ];
        let users: [User; PLAYER_NUMBER] = players.map(|player| {
            let Some(player) = player else {unreachable!()};
            User::default()
                .human(player != ID_PLAYER_BOT)
                .with_id(player)
        });

        println!("{}", serde_json::to_string(&users).unwrap());
    }
}
