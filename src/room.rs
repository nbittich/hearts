use std::{
    collections::HashSet,
    error::Error,
    fmt::{Debug, Display},
    sync::Arc,
};

use crate::constants::{ABRITRATRY_CHANNEL_SIZE, DEFAULT_HANDS, ID_PLAYER_BOT};
use arraystring::ArrayString;
use lib_hearts::{
    get_card_by_idx, Card, Game, GameError, GameState, PlayerState, PositionInDeck, TypeCard,
    PLAYER_CARD_SIZE, PLAYER_NUMBER,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    RwLock,
};
pub type CardEmoji = ArrayString<typenum::U1>;
pub type CardStack = [Option<(usize, usize)>; PLAYER_NUMBER];

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
    ViewerJoined(UserId),
    GetCards,
    ReceiveCards([Option<PlayerCard>; PLAYER_CARD_SIZE]),
    Replaceards([PlayerCard; lib_hearts::NUMBER_REPLACEABLE_CARDS]),
    NewHand {
        player_ids_in_order: [UserId; PLAYER_NUMBER],
        current_player_id: UserId,
        current_hand: u8,
        hands: u8,
    },
    NextPlayerToReplaceCards {
        current_player_id: UserId,
    },
    NextPlayerToPlay {
        current_player_id: UserId,
        stack: [Option<PlayerCard>; PLAYER_NUMBER],
    },
    End,
    PlayerError(GameError),
    Play(PlayerCard),
    GetCurrentState,
    State {
        player_scores: [PlayerState; PLAYER_NUMBER],
        current_cards: [Option<PlayerCard>; PLAYER_CARD_SIZE],
        current_stack: [Option<PlayerCard>; PLAYER_NUMBER],
        current_hand: u8,
        hands: u8,
    },
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
    pub viewers: HashSet<UserId>,
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
            viewers: HashSet::with_capacity(5),
            sender,
            receiver,
        }
    }
}

fn convert_card_to_player_card(card: Option<(usize, &Card)>) -> Option<PlayerCard> {
    if let Some((position_in_deck, card)) = card {
        let emoji: ArrayString<typenum::U1> = ArrayString::from_utf8(card.get_emoji()).unwrap();
        Some(PlayerCard {
            emoji,
            position_in_deck,
            type_card: *card.get_type(),
        })
    } else {
        None
    }
}
fn convert_stack_to_card_player_card(stack: &CardStack) -> [Option<PlayerCard>; PLAYER_NUMBER] {
    stack.map(|s| {
        if let Some((_, card_idx)) = s {
            convert_card_to_player_card(Some((card_idx, get_card_by_idx(card_idx))))
        } else {
            None
        }
    })
}
pub async fn room_task(room: Arc<RwLock<Room>>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (sender, mut receiver) = {
        let room_guard = room.read().await;
        let sender = &room_guard.sender;
        let sender = sender.clone();
        let receiver = sender.subscribe();
        (sender, receiver)
    };
    let room = room.clone();
    while let Ok(msg) = receiver.recv().await {
        let Some(from_user_id) = msg.from_user_id else {continue};
        match msg.msg_type {
            RoomMessageType::Join => {
                let mut room_guard = room.write().await;
                if let &mut RoomState::WaitingForPlayers(mut players) = &mut room_guard.state {
                    if let Some(player_slot) = players.iter_mut().find(|p| p.is_none()) {
                        *player_slot = Some(from_user_id);
                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::Joined(from_user_id),
                        })?;
                    } else {
                        let users: [User; PLAYER_NUMBER] = players.map(|player| {
                            let Some(player) = player else {unreachable!()};
                            User::default()
                                .human(player != ID_PLAYER_BOT)
                                .with_id(player)
                        });

                        let players: [(u64, bool); PLAYER_NUMBER] =
                            users.map(|user| (user.id, user.bot));

                        let game = Game::new(players, DEFAULT_HANDS);
                        let current_player_id =
                            game.current_player_id().ok_or("should not happen")?;

                        let player_ids_in_order = game.player_ids_in_order();
                        room_guard.state = RoomState::Started(users, game);

                        // notify game is about to start

                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::NewHand {
                                player_ids_in_order,
                                current_player_id,
                                current_hand: game.current_hand + 1,
                                hands: game.hands,
                            },
                        })?;
                    }
                } else {
                    room_guard.viewers.insert(from_user_id);
                    sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: None,
                        msg_type: RoomMessageType::ViewerJoined(from_user_id),
                    })?;
                }
            }
            RoomMessageType::GetCards => {
                let room_guard = room.read().await;
                if let RoomState::Started(users, game) = &room_guard.state {
                    let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = game
                        .get_player_cards(from_user_id)
                        .map(convert_card_to_player_card);
                    sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: Some(from_user_id),
                        msg_type: RoomMessageType::ReceiveCards(cards),
                    })?;
                }
            }
            RoomMessageType::Replaceards(player_cards_exchange) => {
                let mut room_guard = room.write().await;
                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if let GameState::ExchangeCards { commands: _ } = &game.state {
                        if game.current_player_id() == Some(from_user_id) {
                            let command = player_cards_exchange.map(|pc| pc.position_in_deck);
                            if let Err(game_error) = game.exchange_cards(command) {
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: Some(from_user_id),
                                    msg_type: RoomMessageType::PlayerError(game_error),
                                })?;
                            } else {
                                let Some(next_player_id) = game.current_player_id() else {unreachable!()};

                                match &game.state {
                                    GameState::ExchangeCards { commands: _ } => {
                                        // send change cards
                                        sender.send(RoomMessage {
                                            from_user_id: None,
                                            to_user_id: None,
                                            msg_type: RoomMessageType::NextPlayerToReplaceCards {
                                                current_player_id: next_player_id,
                                            },
                                        })?;
                                    }
                                    GameState::PlayingHand {
                                        stack,
                                        current_scores: _,
                                    } => {
                                        // send play event
                                        sender.send(RoomMessage {
                                            from_user_id: None,
                                            to_user_id: None,
                                            msg_type: RoomMessageType::NextPlayerToPlay {
                                                current_player_id: next_player_id,
                                                stack: convert_stack_to_card_player_card(stack),
                                            },
                                        })?;
                                    }
                                    any => {
                                        tracing::warn!("receiving weird event from game after exchange cards: {any:?}");
                                    }
                                }
                            }
                        }
                    }
                }
            }
            RoomMessageType::Play(player_card) => {
                let mut room_guard = room.write().await;
                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if let GameState::PlayingHand {
                        stack: _,
                        current_scores: _,
                    } = &game.state
                    {
                        if let Err(game_error) = game.play(player_card.position_in_deck) {
                            sender.send(RoomMessage {
                                from_user_id: None,
                                to_user_id: Some(from_user_id),
                                msg_type: RoomMessageType::PlayerError(game_error),
                            })?;
                        } else {
                            match game.state {
                                GameState::PlayingHand {
                                    stack,
                                    current_scores,
                                } => {
                                    let Some(current_player_id) = game.current_player_id() else {unreachable!()};
                                    sender.send(RoomMessage {
                                        from_user_id: None,
                                        to_user_id: None,
                                        msg_type: RoomMessageType::NextPlayerToPlay {
                                            current_player_id,
                                            stack: convert_stack_to_card_player_card(&stack),
                                        },
                                    })?;
                                }
                                GameState::ComputeScore {
                                    stack,
                                    current_scores,
                                } => {
                                    game.compute_score()?;
                                    match &game.state {
                                        GameState::EndHand => {
                                            game.deal_cards()?;
                                            let current_player_id = game
                                                .current_player_id()
                                                .ok_or("should not happen")?;

                                            let player_ids_in_order = game.player_ids_in_order();
                                            sender.send(RoomMessage {
                                                from_user_id: None,
                                                to_user_id: None,
                                                msg_type: RoomMessageType::NewHand {
                                                    player_ids_in_order,
                                                    current_player_id,
                                                    hands: game.hands,
                                                    current_hand: game.current_hand + 1,
                                                },
                                            })?;
                                        }
                                        GameState::End => {
                                            room_guard.state = RoomState::Done(*players, *game);
                                            sender.send(RoomMessage {
                                                from_user_id: None,
                                                to_user_id: None,
                                                msg_type: RoomMessageType::End,
                                            })?;
                                        }
                                        _ => unreachable!("this cannot happen brazza"),
                                    }
                                }

                                _ => unreachable!("this cannot happen too brazza"),
                            }
                        }
                    }
                }
            }

            RoomMessageType::GetCurrentState => {
                let room_guard = room.read().await;
                match &room_guard.state {
                    RoomState::WaitingForPlayers(_) => {
                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: Some(from_user_id),
                            msg_type: RoomMessageType::PlayerError(GameError::StateError),
                        })?;
                    }
                    RoomState::Started(players, game) | RoomState::Done(players, game) => {
                        // send current state
                        let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = game
                            .get_player_cards(from_user_id)
                            .map(convert_card_to_player_card);
                        let stack = match &game.state {
                            GameState::PlayingHand {
                                stack,
                                current_scores: _,
                            }
                            | GameState::ComputeScore {
                                stack,
                                current_scores: _,
                            } => convert_stack_to_card_player_card(stack),
                            _ => [None; PLAYER_NUMBER],
                        };

                        let scores = game.player_score_by_id();
                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: Some(from_user_id),
                            msg_type: RoomMessageType::State {
                                player_scores: scores,
                                current_cards: cards,
                                current_stack: stack,
                                current_hand: game.current_hand,
                                hands: game.hands,
                            },
                        })?;
                    }
                }
            }
            RoomMessageType::Joined(_) | RoomMessageType::ViewerJoined(_) => {
                tracing::warn!("received joined event. should never happen in theory")
            }
            RoomMessageType::State {
                player_scores: _,
                current_cards: _,
                current_stack: _,
                current_hand: _,
                hands: _,
            } => {
                tracing::warn!("received state event. should never happen in theory")
            }
            RoomMessageType::NewHand {
                player_ids_in_order: _,
                current_player_id: _,
                hands: _,
                current_hand: _,
            } => {
                tracing::warn!("received new hand event. should never happen in theory")
            }
            RoomMessageType::ReceiveCards(_) => {
                tracing::warn!("received receiveCards event. should never happen in theory")
            }
            RoomMessageType::PlayerError(_) => {
                tracing::warn!("received playerError event. should never happen in theory")
            }
            RoomMessageType::NextPlayerToReplaceCards {
                current_player_id: _,
            } => tracing::warn!(
                "received nextPlayerToReplaceCards event. should never happen in theory"
            ),
            RoomMessageType::NextPlayerToPlay {
                current_player_id: _,
                stack: _,
            } => tracing::warn!("received nextPlayerToPlay event. should never happen in theory"),
            RoomMessageType::End => {
                tracing::warn!("received end event. should never happen in theory")
            }
        }
    }
    Ok(())
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
