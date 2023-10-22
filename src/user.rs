use std::sync::Arc;

use arraystring::ArrayString;
use async_session::{async_trait, SessionStore};
use axum::{
    extract::{rejection::TypedHeaderRejectionReason, FromRef, FromRequestParts},
    headers,
    http::{header, request::Parts},
    TypedHeader,
};
use dashmap::DashSet;
use rand::RngCore;
use serde::Serialize;
use serde_derive::Deserialize;
use uuid::Uuid;

use crate::{
    constants::{COOKIE, USER_ID},
    router::AppState,
    utils::HomePageRedirect,
};

pub type UserId = Uuid;

pub type Users = Arc<DashSet<User>>;
#[derive(Copy, Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub name: ArrayString<typenum::U12>,
    pub bot: bool,
}
impl Default for User {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let id = Uuid::new_v4();
        User {
            id,
            name: ArrayString::from_chars(format!("Bot{}", rng.next_u32()).chars()),
            bot: true,
        }
    }
}

impl User {
    pub fn human(self, is_human: bool) -> Self {
        Self {
            bot: !is_human,
            ..self
        }
    }
    pub fn with_id(self, id: UserId) -> Self {
        Self { id, ..self }
    }
    pub fn name(self, name: String) -> Self {
        Self {
            name: ArrayString::from_chars(name.chars()),
            ..self
        }
    }
}

#[async_trait]
impl<B> FromRequestParts<B> for User
where
    AppState: FromRef<B>,
    B: Send + Sync,
{
    // If anything goes wrong or no session is found, redirect to the auth page
    type Rejection = HomePageRedirect;

    async fn from_request_parts(req: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        match TypedHeader::<headers::Cookie>::from_request_parts(req, state)
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => HomePageRedirect,
                    _ => {
                        tracing::error!("unexpected error getting Cookie header(s): {}", e);
                        HomePageRedirect
                    }
                },
                _ => {
                    tracing::error!("unexpected error getting cookies: {}", e);
                    HomePageRedirect
                }
            }) {
            Ok(TypedHeader(cookies)) => {
                let session_cookie = cookies.get(COOKIE).ok_or(HomePageRedirect)?;
                let session = app_state
                    .store
                    .load_session(session_cookie.to_string())
                    .await
                    .ok()
                    .flatten()
                    .ok_or(HomePageRedirect)?;

                let user_id = session.get::<UserId>(USER_ID).ok_or(HomePageRedirect)?;
                app_state
                    .users
                    .iter()
                    .find_map(|u| {
                        if u.id == user_id {
                            Some(u.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or(HomePageRedirect)
            }
            Err(e) => Err(e),
        }
    }
}
