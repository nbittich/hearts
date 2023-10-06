use std::fmt::Display;

use axum::{
    http::{header::SET_COOKIE, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Local;
use constants::COOKIE as COOKIE_NAME;

use crate::constants;
#[derive(Debug)]
pub struct HomePageRedirect;

impl IntoResponse for HomePageRedirect {
    fn into_response(self) -> Response {
        tracing::debug!("in case of an error, remove cookie");
        let mut resp = Redirect::temporary("/").into_response();
        let now = Local::now().to_rfc2822();
        let cookie = format!("{}=; SameSite=Lax; Path=/; expires={}", COOKIE_NAME, now);
        match cookie.parse() {
            Ok(cookie) => {
                resp.headers_mut().insert(SET_COOKIE, cookie);
                resp
            }
            Err(e) => {
                tracing::error!("could not parse cookie. {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
pub fn service_error(e: impl Display) -> impl IntoResponse {
    tracing::error!("service error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[cfg(test)]
mod test {
    use lib_hearts::PLAYER_NUMBER;
    use uuid::Uuid;

    use crate::user::User;

    #[test]
    fn test_to_static_array1() {
        let players: [Option<Uuid>; PLAYER_NUMBER] = [
            Some(Uuid::from_u128(1)),
            Some(Uuid::from_u128(2)),
            Some(Uuid::from_u128(3)),
            Some(Uuid::from_u128(4)),
        ];
        let users: [User; PLAYER_NUMBER] = players.map(|player| {
            let Some(player) = player else { unreachable!() };
            User::default()
                .human(player != Uuid::from_u128(0))
                .with_id(player)
        });

        println!("{}", serde_json::to_string(&users).unwrap());
    }
}
