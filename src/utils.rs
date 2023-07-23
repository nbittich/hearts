use std::fmt::Display;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};

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

#[cfg(test)]
mod test {
    use lib_hearts::PLAYER_NUMBER;

    use crate::{constants::ID_PLAYER_BOT, room::User};

    #[test]
    fn test_to_static_array1() {
        let players: [Option<u64>; PLAYER_NUMBER] = [Some(1), Some(2), Some(3), Some(4)];
        let users: [User; PLAYER_NUMBER] = players.map(|player| {
            let Some(player) = player else {unreachable!()};
            User::default()
                .human(player != ID_PLAYER_BOT)
                .with_id(player)
        });

        dbg!(serde_json::to_string(&users).unwrap());
    }
}
