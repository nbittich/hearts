use std::{
    collections::HashSet,
    error::Error,
    fmt::{Debug, Display},
    sync::Arc,
};

use crate::constants::{ABRITRATRY_CHANNEL_SIZE, DEFAULT_HANDS};
use arraystring::ArrayString;
use lib_hearts::{
    get_card_by_idx, Card, Game, GameError, GameState, PlayerState, PositionInDeck, TypeCard,
    PLAYER_CARD_SIZE, PLAYER_NUMBER,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        broadcast::{self, Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
};
use uuid::Uuid;
pub type CardEmoji = ArrayString<typenum::U1>;
pub type CardStack = [Option<(usize, usize)>; PLAYER_NUMBER];
pub type Rooms = Arc<RwLock<Vec<Arc<RwLock<Room>>>>>;
pub type Users = Arc<RwLock<Vec<User>>>;
#[derive(Serialize, Copy, Clone, Debug, Deserialize)]
pub struct PlayerCard {
    pub type_card: TypeCard,
    pub emoji: CardEmoji,
    pub position_in_deck: PositionInDeck,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RoomMessageType {
    Join,
    Joined(UserId),
    ViewerJoined(UserId),
    GetCards,
    ReceiveCards([Option<PlayerCard>; PLAYER_CARD_SIZE]),
    ReplaceCards([PlayerCard; lib_hearts::NUMBER_REPLACEABLE_CARDS]),
    ReplaceCardsBot,
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
    PlayBot,
    GetCurrentState,
    State {
        player_scores: [PlayerState; PLAYER_NUMBER],
        current_cards: [Option<PlayerCard>; PLAYER_CARD_SIZE],
        current_stack: [Option<PlayerCard>; PLAYER_NUMBER],
        current_hand: u8,
        hands: u8,
    },
    WaitingForPlayers([Option<UserId>; PLAYER_NUMBER]),
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct RoomMessage {
    // if from_user_id is none, the message comes from system
    // if to_user_id is none and from_user_id is some, the message is for system
    // if both are none, the message should be broadcast
    pub from_user_id: Option<UserId>,
    #[serde(skip_deserializing)]
    pub to_user_id: Option<UserId>,
    pub msg_type: RoomMessageType,
}

pub type UserId = Uuid;

#[derive(Serialize)]
pub struct Room {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub state: RoomState,
    pub viewers: HashSet<UserId>,
    #[serde(skip_serializing)]
    pub sender: Sender<RoomMessage>,
    #[serde(skip_serializing)]
    pub receiver: Receiver<RoomMessage>,
    #[serde(skip_serializing)]
    pub task: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
}

pub enum RoomState {
    WaitingForPlayers([Option<UserId>; PLAYER_NUMBER]),
    Started([User; PLAYER_NUMBER], Game),
    Done([User; PLAYER_NUMBER], Game),
}

#[derive(Copy, Clone, Debug, Serialize)]
pub struct User {
    id: UserId,
    name: ArrayString<typenum::U12>,
    bot: bool,
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

// only JOIN and get state are allowed for viewers
fn is_valid_msg(room: &Room, user_id: UserId) -> bool {
    !room.viewers.contains(&user_id)
}

impl Room {
    pub async fn new(users: Users) -> Arc<RwLock<Room>> {
        let (sender, receiver) = broadcast::channel(ABRITRATRY_CHANNEL_SIZE);
        let id = Uuid::new_v4();
        let room = Room {
            id,
            state: RoomState::WaitingForPlayers([None; PLAYER_NUMBER]),
            viewers: HashSet::with_capacity(5),
            sender,
            receiver,
            task: None,
        };
        let room = Arc::new(RwLock::new(room));

        let (clone, room) = (room.clone(), room);
        let task = tokio::spawn(room_task(clone, users, id));

        let mut room_guard = room.write().await;
        room_guard.task = Some(task);

        room.clone()
    }
}

// send messages and check state after each play
// if game is done, return true
async fn send_message_after_played(
    game: &mut Game,
    sender: &Sender<RoomMessage>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
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
                    let current_player_id = game.current_player_id().ok_or("should not happen")?;

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
                GameState::End => return Ok(true),
                _ => unreachable!("this cannot happen brazza"),
            }
        }

        _ => unreachable!("this cannot happen too brazza"),
    }
    Ok(false)
}
async fn send_message_after_cards_replaced(
    game: &Game,
    sender: &Sender<RoomMessage>,
    next_player_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    Ok(())
}
pub async fn room_task(
    room: Arc<RwLock<Room>>,
    users: Users,
    id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing::info!("setup room task {id}...");
    let (sender, mut receiver) = {
        let room_guard = room.read().await;
        let sender = room_guard.sender.clone();
        let receiver = sender.subscribe();
        (sender, receiver)
    };
    let room = room.clone();
    tracing::info!("listening task {id}...");
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
                        let users_guard = users.read().await;
                        let users: [User; PLAYER_NUMBER] = players.map(|player| {
                            let Some(player) = player else {unreachable!()};
                            users_guard
                                .iter()
                                .find(|p| p.id == player)
                                .cloned()
                                .unwrap_or_else(|| User::default().human(false).with_id(player))
                        });

                        let players: [(UserId, bool); PLAYER_NUMBER] =
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
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
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

            RoomMessageType::ReplaceCardsBot => {
                let mut room_guard = room.write().await;

                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if game.current_player_id() == Some(from_user_id)
                    // we don't check if player
                    // is a bot or not, in order to be able to implement timeout later
                    {
                        if let GameState::ExchangeCards { commands: _ } = &game.state {
                            game.play_bot()?;
                            let Some(next_player_id) = game.current_player_id() else {unreachable!()};
                            send_message_after_cards_replaced(game, &sender, next_player_id)
                                .await?;
                        }
                    }
                }
            }

            RoomMessageType::ReplaceCards(player_cards_exchange) => {
                let mut room_guard = room.write().await;
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if game.current_player_id() == Some(from_user_id) {
                        if let GameState::ExchangeCards { commands: _ } = &game.state {
                            let command = player_cards_exchange.map(|pc| pc.position_in_deck);
                            if let Err(game_error) = game.exchange_cards(command) {
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: Some(from_user_id),
                                    msg_type: RoomMessageType::PlayerError(game_error),
                                })?;
                            } else {
                                let Some(next_player_id) = game.current_player_id() else {unreachable!()};
                                send_message_after_cards_replaced(game, &sender, next_player_id)
                                    .await?;
                            }
                        }
                    }
                }
            }
            RoomMessageType::PlayBot => {
                let mut room_guard = room.write().await;

                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if game.current_player_id() == Some(from_user_id)
                    // we don't check if player
                    // is a bot or not, in order to be able to implement timeout later
                    {
                        if let GameState::PlayingHand {
                            stack: _,
                            current_scores: _,
                        } = &game.state
                        {
                            game.play_bot()?;
                            if send_message_after_played(game, &sender).await? {
                                // game is done, update state
                                room_guard.state = RoomState::Done(*players, *game);
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::End,
                                })?;
                            }
                        }
                    }
                }
            }
            RoomMessageType::Play(player_card) => {
                let mut room_guard = room.write().await;
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
                if let RoomState::Started(players, game) = &mut room_guard.state {
                    if game.current_player_id() == Some(from_user_id) {
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
                            } else if send_message_after_played(game, &sender).await? {
                                // game is done, update state
                                room_guard.state = RoomState::Done(*players, *game);
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::End,
                                })?;
                            }
                        }
                    }
                }
            }
            RoomMessageType::GetCurrentState => {
                let room_guard = room.read().await;
                match &room_guard.state {
                    RoomState::WaitingForPlayers(players_slot) => {
                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: Some(from_user_id),
                            msg_type: RoomMessageType::WaitingForPlayers(*players_slot),
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
            e => {
                tracing::warn!("received {e:?}. should not happen");
                continue;
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

    use uuid::Uuid;

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
               "from_user_id": "96f6b528-4fdc-47ed-8c50-277b13587fc1",
               "to_user_id" : "96f6b528-4fdc-47ed-8c50-277b13587fcÉ",
               "msg_type": "JOIN"
            }
            "#
            )
            .unwrap()
        );
    }
    #[test]
    fn test_serialize_msg() {
        println!(
            "{}",
            serde_json::to_string_pretty(&RoomMessage {
                from_user_id: Some(Uuid::new_v4()),
                to_user_id: Some(Uuid::new_v4()),
                msg_type: crate::room::RoomMessageType::Joined(Uuid::new_v4())
            })
            .unwrap()
        );
    }
}
