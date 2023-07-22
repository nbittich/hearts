use std::{fmt::Display, mem::MaybeUninit};

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

pub unsafe fn to_static_array<I, O, F, const N: usize>(inputs: &[I], transform: F) -> [O; N]
where
    I: Copy,
    O: Copy,
    F: Fn(I) -> O,
{
    let mut outputs: [MaybeUninit<O>; N] = MaybeUninit::uninit().assume_init();
    for (idx, output) in outputs.iter_mut().enumerate() {
        output.write(transform(inputs[idx]));
    }

    *(&outputs as *const [MaybeUninit<O>; N] as *const [O; N])
}
#[cfg(test)]
mod test {
    use lib_hearts::PLAYER_NUMBER;

    use crate::{constants::ID_PLAYER_BOT, room::User};

    use super::to_static_array;

    #[test]
    fn test_to_static_array1() {
        let players: [Option<u64>; PLAYER_NUMBER] = [Some(1), Some(2), Some(3), Some(4)];
        let users: [User; PLAYER_NUMBER] = unsafe {
            to_static_array(&players[..], |player| {
                let Some(player) = player else {unreachable!()};
                User::default()
                    .human(player != ID_PLAYER_BOT)
                    .with_id(player)
            })
        };

        dbg!(serde_json::to_string(&users).unwrap());
    }
}
