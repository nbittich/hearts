use std::{borrow::Cow, net::SocketAddr, ops::ControlFlow};

use async_session::{MemoryStore, SessionStore};
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket},
        ConnectInfo, Path, State, WebSocketUpgrade,
    },
    headers,
    http::StatusCode,
    response::{ErrorResponse, IntoResponse},
    TypedHeader,
};
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use tokio::sync::broadcast::Sender;
use uuid::Uuid;

use crate::{
    constants::{COOKIE, USER_ID},
    room::{RoomMessage, Rooms, UserId},
    utils::{service_error, HomePageRedirect},
};

pub async fn ws_handler(
    Path(room_id): Path<Uuid>,
    State(store): State<MemoryStore>,
    State(rooms): State<Rooms>,
    ws: WebSocketUpgrade,
    cookies: Option<TypedHeader<headers::Cookie>>,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum::response::Result<impl IntoResponse> {
    let cookies = cookies.ok_or_else(|| service_error("cookie not present"))?;
    let session_cookie = cookies.get(COOKIE).ok_or(HomePageRedirect)?;
    let session = store
        .load_session(session_cookie.to_string())
        .await
        .ok()
        .flatten()
        .ok_or(HomePageRedirect)?;

    let user_id = session.get::<UserId>(USER_ID).ok_or(HomePageRedirect)?;
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    tracing::info!("`{user_id} with agent {user_agent}` at {addr} connected.");
    let rooms_guard = rooms.read().await;

    for r in rooms_guard.iter() {
        let room = r.read().await;
        if room.id == room_id {
            let sender = room.sender.clone();

            return axum::response::Result::Ok(
                ws.on_upgrade(move |socket| handle_socket(socket, addr, sender, user_id)),
            );
        }
    }
    Err(ErrorResponse::from(StatusCode::NOT_FOUND))
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    user_sender: Sender<RoomMessage>,
    user_id: UserId,
) {
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        tracing::info!("Pinged {}...", who);
    } else {
        tracing::info!("Could not send ping {}!", who);
        return;
    }

    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if process_message(&msg, who).is_break() {
                return;
            }
        } else {
            tracing::info!("client {who} abruptly disconnected");
            return;
        }
    }

    let mut user_receiver = user_sender.subscribe();

    let (mut sender, mut receiver) = socket.split();

    let mut room_send_task = tokio::spawn(async move {
        async fn send_msg(
            sender: &mut SplitSink<WebSocket, Message>,
            msg: RoomMessage,
        ) -> ControlFlow<()> {
            let Ok(msg) = serde_json::to_string(&msg) else{
                    tracing::error!("could not serialize msg {msg:?}!!");
                    return ControlFlow::Break(());
                };
            if let Err(e) = sender.send(Message::Text(msg)).await {
                tracing::error!("Could not send message back due to {e}!!!");
                return ControlFlow::Break(());
            }
            ControlFlow::Continue(())
        }

        while let Ok(msg) = user_receiver.recv().await {
            if let Some(to_user_id) = &msg.to_user_id {
                // check if the message is for this user
                if to_user_id != &user_id {
                    // it is not for you
                    continue;
                }
                if send_msg(&mut sender, msg).await.is_break() {
                    break;
                };
            } else if msg.from_user_id.is_none() {
                // message comes from system and is not for anyone in particular, broadcast it
                if send_msg(&mut sender, msg).await.is_break() {
                    break;
                };
            }
        }
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: Cow::from("Goodbye"),
            })))
            .await
        {
            tracing::warn!("Could not send Close due to {}, probably it is ok?", e);
        }
    });
    let mut room_receive_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // print message and break if instructed to do so
            match process_message(&msg, who) {
                ControlFlow::Continue(Some(room_msg)) => {
                    if let Err(e) = user_sender.send(room_msg) {
                        tracing::error!("could not send message to room {e:?}, message: {msg:?}");
                        break;
                    }
                }
                ControlFlow::Continue(None) => {
                    tracing::warn!("continue... although message wasn't a room message :{msg:?}");
                }
                ControlFlow::Break(_) => break,
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_a = (&mut room_send_task) => {
            match rv_a {
                Ok(_) => tracing::info!("stop sending messages to {who}"),
                Err(a) => tracing::info!("Error sending messages {:?}", a)
            }
            room_receive_task.abort();
        },
        rv_b = (&mut room_receive_task) => {
            match rv_b {
                Ok(_) => tracing::info!("stop receiving messages from {who}"),
                Err(b) => tracing::info!("Error receiving messages {b:?}")
            }
            room_send_task.abort();
        }
    }

    // returning from the handler closes the websocket connection
    tracing::info!("Websocket context {} destroyed", who);
}

fn process_message(msg: &Message, who: SocketAddr) -> ControlFlow<(), Option<RoomMessage>> {
    match msg {
        Message::Text(t) => {
            tracing::debug!(">>> {} sent str: {:?}", who, t);
            match serde_json::from_str::<RoomMessage>(t) {
                Ok(msg) => return ControlFlow::Continue(Some(msg)),
                Err(e) => {
                    tracing::error!("could not deserialize message {e}");
                    return ControlFlow::Break(());
                }
            }
        }
        Message::Binary(d) => {
            tracing::error!(
                "Binary message was sent by {who} with a length of {} bytes: {d:?}",
                d.len(),
            );
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                tracing::info!(
                    ">>> {} sent close with code {} and reason `{}`",
                    who,
                    cf.code,
                    cf.reason
                );
            } else {
                tracing::info!(">>> {} somehow sent close message without CloseFrame", who);
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            tracing::info!(">>> {} sent pong with {:?}", who, v);
        }
        Message::Ping(v) => {
            tracing::debug!(">>> {} sent ping with {:?}", who, v);
        }
    }
    ControlFlow::Continue(None)
}
