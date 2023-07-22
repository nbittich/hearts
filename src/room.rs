use std::{error::Error, fmt::Display, sync::Arc};

use crate::{
    constants::{ABRITRATRY_CHANNEL_SIZE, DEFAULT_HANDS, ID_PLAYER_BOT},
    utils::to_static_array,
};
use arraystring::ArrayString;
use lib_hearts::{Game, PositionInDeck, TypeCard, PLAYER_CARD_SIZE, PLAYER_NUMBER};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    RwLock,
};
pub type CardEmoji = ArrayString<typenum::U1>;

#[derive(Serialize, Copy, Clone, Debug, Deserialize)]
pub struct PlayerCard {
    pub type_card: TypeCard,
    pub emoji: CardEmoji,
    pub position_in_deck: PositionInDeck,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoomMessageType {
    Join,
    Joined(UserId),
    GameStarting,
    GetCards,
    ReceiveCards([Option<PlayerCard>; PLAYER_CARD_SIZE]),
    ChangeCards,
    Play,
    GetCurrentState,
    State,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct RoomMessage {
    // if from_user_id is none, the message comes from system
    // if to_user_id is none and from_user_id is some, the message is for system
    // if both are none, the message should be broadcast
    pub from_user_id: Option<u64>,
    #[serde(skip_deserializing)]
    pub to_user_id: Option<u64>,
    pub msg_type: RoomMessageType,
}

pub type UserId = u64;

#[derive(Serialize)]
pub struct Room {
    pub id: String,
    #[serde(skip_serializing)]
    pub state: RoomState,
    pub viewers: Vec<UserId>,

    #[serde(skip_serializing)]
    pub sender: Sender<RoomMessage>,
    #[serde(skip_serializing)]
    pub receiver: Receiver<RoomMessage>,
}

pub enum RoomState {
    WaitingForPlayers([Option<UserId>; PLAYER_NUMBER]),
    Started([User; PLAYER_NUMBER], Game),
    Done([User; PLAYER_NUMBER], Game),
}

#[derive(Copy, Clone, Debug, Serialize)]
pub struct User {
    id: u64,
    name: ArrayString<typenum::U12>,
    bot: bool,
}

impl Default for User {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let id = rng.next_u64();
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
    pub fn with_id(self, id: u64) -> Self {
        Self { id, ..self }
    }
}

impl Default for Room {
    fn default() -> Self {
        let (sender, receiver) = broadcast::channel(ABRITRATRY_CHANNEL_SIZE);
        Room {
            id: uuid::Uuid::new_v4().to_string(),
            state: RoomState::WaitingForPlayers([None; PLAYER_NUMBER]),
            viewers: Vec::with_capacity(5),
            sender,
            receiver,
        }
    }
}

pub async fn room_task(room: Arc<RwLock<Room>>) {
    let room_guard = room.read().await;
    let sender = &room_guard.sender;
    let sender = sender.clone();
    let mut receiver = sender.subscribe();
    drop(room_guard); // we don't want to keep the guard
    let room = room.clone();
    while let Ok(msg) = receiver.recv().await {
        let Some(from_user_id) = msg.from_user_id else {continue};
        match msg.msg_type {
            RoomMessageType::Join => {
                let mut room_guard = room.write().await;
                if let &mut RoomState::WaitingForPlayers(mut players) = &mut room_guard.state {
                    if let Some(player_slot) = players.iter_mut().find(|p| p.is_none()) {
                        *player_slot = Some(from_user_id);
                        if let Err(e) = sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::Joined(from_user_id),
                        }) {
                            tracing::error!("could not send message to room. kill the room {e:?}");
                            break;
                        };
                    } else {
                        let users: [User; PLAYER_NUMBER] = unsafe {
                            to_static_array(&players[..], |player| {
                                let Some(player) = player else {unreachable!()};
                                User::default()
                                    .human(player != ID_PLAYER_BOT)
                                    .with_id(player)
                            })
                        };

                        let players: [(u64, bool); PLAYER_NUMBER] =
                            unsafe { to_static_array(&users, |user| (user.id, user.bot)) };

                        let game = Game::new(players, DEFAULT_HANDS);
                        room_guard.state = RoomState::Started(users, game);
                        // notify game is about to start

                        if let Err(e) = sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::GameStarting,
                        }) {
                            tracing::error!("could not send message to room. kill the room {e:?}");
                            break;
                        };
                    }
                }
            }
            RoomMessageType::GetCards => {
                let room_guard = room.read().await;
                if let RoomState::Started(users, game) = &room_guard.state {
                    let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = unsafe {
                        to_static_array(&game.get_player_cards(from_user_id), |card| {
                            if let Some((position_in_deck, card)) = card {
                                let emoji: ArrayString<typenum::U1> =
                                    ArrayString::from_utf8(card.get_emoji()).unwrap();
                                Some(PlayerCard {
                                    emoji,
                                    position_in_deck,
                                    type_card: *card.get_type(),
                                })
                            } else {
                                None
                            }
                        })
                    };
                    if let Err(e) = sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: Some(from_user_id),
                        msg_type: RoomMessageType::ReceiveCards(cards),
                    }) {
                        tracing::error!("could not send message to room. kill the room {e:?}");
                        break;
                    };
                }
            }
            RoomMessageType::Joined(_) => {
                tracing::warn!("received joined event. should never happen in theory")
            }
            RoomMessageType::State => {
                tracing::warn!("received state even. should never happen in theory")
            }
            RoomMessageType::ReceiveCards(_) => {
                tracing::warn!("received receiveCards event. should never happen in theory")
            }
            RoomMessageType::ChangeCards => todo!(),
            RoomMessageType::Play => todo!(),
            RoomMessageType::GetCurrentState => todo!(),
            RoomMessageType::GameStarting => todo!(),
        }
    }
}

#[derive(Debug)]
pub struct RoomError(String);
impl Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for RoomError {}

impl RoomError {
    fn from(e: impl Display) -> Self {
        RoomError(e.to_string())
    }
}

#[cfg(test)]
mod test {
    use crate::room::RoomMessage;

    use super::User;

    #[test]
    fn test_serializ_user() {
        println!(
            "{}",
            serde_json::to_string_pretty(&User::default()).unwrap()
        );
    }
    #[test]
    fn test_skip_deserializing() {
        println!(
            "{:?}",
            serde_json::from_str::<RoomMessage>(
                r#"
            {
               "from_user_id": 98,
               "to_user_id" : 92,
               "msg_type": "JOIN"
            }
            "#
            )
        );
    }
    #[test]
    fn test_serialize_msg() {
        println!(
            "{}",
            serde_json::to_string_pretty(&RoomMessage {
                from_user_id: Some(123),
                to_user_id: Some(456),
                msg_type: crate::room::RoomMessageType::Joined(123)
            })
            .unwrap()
        );
    }
}
