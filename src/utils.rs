use std::{error::Error, fmt::Display, mem::MaybeUninit};

use crate::constants;
use axum::{
    http::{header::SET_COOKIE, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Local;
use constants::COOKIE as COOKIE_NAME;
use futures_util::Future;
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

#[derive(Debug)]
struct InternalError<T>(T);
impl<T: Send + Sync + std::fmt::Debug> Display for InternalError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl<T: Send + Sync + std::fmt::Debug> Error for InternalError<T> {}
pub async fn to_static_array<I, O, F, Fut, const N: usize>(
    inputs: &[I],
    transform: F,
) -> Result<[O; N], Box<dyn Error + Send + Sync>>
where
    I: Copy,
    O: Copy,
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O, Box<dyn Error + Send + Sync>>>,
{
    if inputs.len() != N {
        return Err(Box::new(InternalError("input len != output len")));
    }

    // Safety: trust me bro
    unsafe {
        let mut outputs: [MaybeUninit<O>; N] = MaybeUninit::uninit().assume_init();
        for (idx, output) in outputs.iter_mut().enumerate() {
            output.write(transform(inputs[idx]).await?);
        }

        Ok(*(&outputs as *const [MaybeUninit<O>; N] as *const [O; N]))
    }
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
