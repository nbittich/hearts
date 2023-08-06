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
            let Some(player) = player else {unreachable!()};
            User::default()
                .human(player != Uuid::from_u128(0))
                .with_id(player)
        });

        println!("{}", serde_json::to_string(&users).unwrap());
    }
}
