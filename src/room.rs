use std::{
    borrow::Cow,
    collections::HashSet,
    error::Error,
    fmt::{Debug, Display},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    constants::{ABRITRATRY_CHANNEL_SIZE, DEFAULT_HANDS, TIMEOUT_SECS},
    user::{User, UserId, Users},
};
use arraystring::ArrayString;
use lib_hearts::{
    get_card_by_idx, Card, Game, GameError, GameState, PlayerState, PositionInDeck, TypeCard,
    PLAYER_CARD_SIZE, PLAYER_NUMBER,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        broadcast::{self, Receiver, Sender},
        RwLock,
    },
    task::JoinHandle,
    time::timeout,
};
use uuid::Uuid;
pub type CardEmoji = ArrayString<typenum::U4>;
pub type CardStack = [Option<(usize, usize)>; PLAYER_NUMBER];
pub type Rooms = Arc<RwLock<Vec<Arc<RwLock<Room>>>>>;

const BOT_SLEEP_SECS: u64 = 1;

#[derive(Serialize, Copy, PartialEq, Clone, Debug, Deserialize)]
pub struct PlayerCard {
    pub type_card: TypeCard,
    pub emoji: CardEmoji,
    pub position_in_deck: PositionInDeck,
}

pub type StaticStr = Cow<'static, str>;

#[derive(Clone, Serialize, PartialEq, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum RoomMessageType {
    Join,
    TimedOut,
    JoinBot,
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
        player_scores: [PlayerState; PLAYER_NUMBER],
        hands: u8,
    },
    NextPlayerToReplaceCards {
        current_player_id: UserId,
    },
    NextPlayerToPlay {
        current_player_id: UserId,
        current_cards: Option<[Option<PlayerCard>; PLAYER_CARD_SIZE]>,
        stack: [Option<PlayerCard>; PLAYER_NUMBER],
    },
    UpdateStackAndScore {
        stack: [Option<PlayerCard>; PLAYER_NUMBER],
        player_scores: [PlayerState; PLAYER_NUMBER],
        current_scores: Option<[PlayerState; PLAYER_NUMBER]>,
    },
    End {
        player_scores: [PlayerState; PLAYER_NUMBER],
    },
    PlayerError(GameError),
    Play(PlayerCard),
    PlayBot,
    GetCurrentState,
    State {
        mode: StaticStr,
        player_scores: [PlayerState; PLAYER_NUMBER],
        current_scores: [PlayerState; PLAYER_NUMBER],
        current_cards: Box<[Option<PlayerCard>; PLAYER_CARD_SIZE]>,
        current_stack: [Option<PlayerCard>; PLAYER_NUMBER],
        current_hand: u8,
        current_player_id: Option<UserId>,
        hands: u8,
    },
    WaitingForPlayers([Option<UserId>; PLAYER_NUMBER]),
}

#[derive(Clone, Serialize, PartialEq, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoomMessage {
    // if from_user_id is none, the message comes from system
    // if to_user_id is none and from_user_id is some, the message is for system
    // if both are none, the message should be broadcast
    pub from_user_id: Option<UserId>,
    #[serde(skip_deserializing)]
    pub to_user_id: Option<UserId>,
    pub msg_type: RoomMessageType,
}

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

fn convert_card_to_player_card(card: Option<(usize, &Card)>) -> Option<PlayerCard> {
    if let Some((position_in_deck, card)) = card {
        let emoji: ArrayString<typenum::U4> = ArrayString::from_utf8(card.get_emoji()).ok()?;
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

async fn timeout_bot(
    player_id: Uuid,
    mut receiver: Receiver<RoomMessage>,
    sender: Sender<RoomMessage>,
    bot_msg: impl Fn() -> RoomMessage,
) {
    let now = Instant::now();
    let mut timeout_act = Duration::from_secs(TIMEOUT_SECS as u64);

    let sub_t = |a: Duration, b: Duration| match a.checked_sub(b) {
        Some(d) => d,
        None => Duration::ZERO,
    };
    loop {
        tracing::debug!("entering timeout loop with a duration of {timeout_act:?}");
        match timeout(timeout_act, receiver.recv()).await {
            Ok(Ok(rm)) => match rm.msg_type {
                RoomMessageType::NewHand {
                    current_player_id, ..
                }
                | RoomMessageType::NextPlayerToReplaceCards { current_player_id }
                | RoomMessageType::NextPlayerToPlay {
                    current_player_id, ..
                } if current_player_id != player_id => {
                    tracing::debug!("it's all good mate. {current_player_id}");

                    return;
                }
                RoomMessageType::End { .. } => {
                    tracing::debug!("game over. timeout bot");
                    return;
                }
                msg => {
                    tracing::debug!("invalid message: {msg:?}");
                    timeout_act = sub_t(timeout_act, now.elapsed());
                }
            },
            Ok(Err(e)) => {
                tracing::debug!("timeout: {e}");
                timeout_act = sub_t(timeout_act, now.elapsed());
            }
            Err(t) => {
                let msg = bot_msg();
                tracing::error!("{player_id} TIMED OUT. attempt to send {msg:?}");

                let res = sender.send(RoomMessage {
                    from_user_id: None,
                    to_user_id: Some(player_id),
                    msg_type: RoomMessageType::TimedOut,
                });

                tracing::debug!("send timed out {res:?}");
                let res = sender.send(bot_msg());
                tracing::debug!("timeout_bot attempts to send msg. res: {res:?}",);
                return;
            }
        }
    }
}

// send messages and check state after each play
// if game is done, return true
async fn send_message_after_played(
    game: &mut Game,
    sender: &Sender<RoomMessage>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    // todo we may need to filter on user that are not bot
    let timeout_sender = sender.clone();
    let timeout_receiver = timeout_sender.subscribe();
    let Some(ref current_player_id) = game.current_player_id() else {
        unreachable!()
    };

    match &mut game.state {
        GameState::PlayingHand { stack, .. } => {
            let current_player_id = *current_player_id;
            sender.send(RoomMessage {
                from_user_id: None,
                to_user_id: None,
                msg_type: RoomMessageType::NextPlayerToPlay {
                    current_player_id,
                    current_cards: None,
                    stack: convert_stack_to_card_player_card(stack),
                },
            })?;
            tokio::spawn(async move {
                timeout_bot(current_player_id, timeout_receiver, timeout_sender, || {
                    RoomMessage {
                        from_user_id: Some(current_player_id),
                        to_user_id: None,
                        msg_type: RoomMessageType::PlayBot,
                    }
                })
                .await
            });
        }
        GameState::ComputeScore { ref stack, .. } => {
            let stack = *stack;

            game.compute_score()?;

            let current_scores = game.current_score_by_id();
            let player_scores = game.player_score_by_id();
            sender.send(RoomMessage {
                from_user_id: None,
                to_user_id: None,
                msg_type: RoomMessageType::UpdateStackAndScore {
                    stack: convert_stack_to_card_player_card(&stack),
                    current_scores: Some(current_scores),
                    player_scores,
                },
            })?;
            tokio::time::sleep(Duration::from_secs(2)).await;

            match &game.state {
                GameState::PlayingHand {
                    stack,
                    current_scores,
                } => {
                    let current_player_id = game.current_player_id().ok_or("No current id")?;
                    sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: None,
                        msg_type: RoomMessageType::NextPlayerToPlay {
                            current_player_id,
                            current_cards: None,
                            stack: convert_stack_to_card_player_card(stack),
                        },
                    })?;

                    tokio::spawn(async move {
                        timeout_bot(current_player_id, timeout_receiver, timeout_sender, || {
                            RoomMessage {
                                from_user_id: Some(current_player_id),
                                to_user_id: None,
                                msg_type: RoomMessageType::PlayBot,
                            }
                        })
                        .await
                    });
                }
                GameState::EndHand | GameState::ExchangeCards { commands: _ } => {
                    game.deal_cards()?;
                    let current_player_id = game.current_player_id().ok_or("should not happen")?;

                    let player_ids_in_order = game.player_ids_in_order();
                    let player_scores = game.player_score_by_id();

                    sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: None,
                        msg_type: RoomMessageType::NewHand {
                            player_ids_in_order,
                            current_player_id,
                            player_scores,
                            hands: game.hands,
                            current_hand: game.current_hand,
                        },
                    })?;
                    tokio::spawn(async move {
                        timeout_bot(current_player_id, timeout_receiver, timeout_sender, || {
                            RoomMessage {
                                from_user_id: Some(current_player_id),
                                to_user_id: None,
                                msg_type: RoomMessageType::ReplaceCardsBot,
                            }
                        })
                        .await
                    });
                }
                GameState::End => return Ok(true), // FIXME probably send something brazza
                e => unreachable!("this cannot happen brazza {e:?}"),
            }
        }

        any => unreachable!("this cannot happen too brazza {any:?}"),
    }
    Ok(false)
}

async fn bot_task(
    sender: Sender<RoomMessage>,
    bot_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing::info!("setup bot task {bot_id}...");
    let mut receiver = sender.subscribe();

    tracing::info!("listening bot {bot_id}...");
    sender.send(RoomMessage {
        from_user_id: Some(bot_id),
        to_user_id: None,
        msg_type: RoomMessageType::Join,
    })?;
    while let Ok(msg) = receiver.recv().await {
        if msg.from_user_id.is_some() {
            continue; // only interested by system msg
        }

        match msg.msg_type {
            RoomMessageType::NewHand {
                player_ids_in_order: _,
                current_player_id,
                current_hand: _,
                hands: _,
                ..
            }
            | RoomMessageType::NextPlayerToReplaceCards { current_player_id }
                if current_player_id == bot_id =>
            {
                tokio::time::sleep(Duration::from_secs(BOT_SLEEP_SECS)).await; // give some delay
                sender.send(RoomMessage {
                    from_user_id: Some(bot_id),
                    to_user_id: None,
                    msg_type: RoomMessageType::ReplaceCardsBot,
                })?;
            }

            RoomMessageType::NextPlayerToPlay {
                current_player_id,
                stack,
                ..
            } if current_player_id == bot_id => {
                tokio::time::sleep(Duration::from_secs(BOT_SLEEP_SECS)).await; // give some delay

                sender.send(RoomMessage {
                    from_user_id: Some(bot_id),
                    to_user_id: None,
                    msg_type: RoomMessageType::PlayBot,
                })?;
            }
            RoomMessageType::End { .. } => {
                tracing::info!("bot {bot_id} say goodbye.");
                return Ok(());
            }

            _ => tracing::debug!("we don't care about {msg:?}"),
        }
        // todo
    }
    Ok(())
}

async fn send_message_after_cards_replaced(
    game: &Game,
    sender: &Sender<RoomMessage>,
    next_player_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // todo we may need to filter on user that are not bot
    // be sure to check both cases where sender/receiver are used
    let timeout_sender = sender.clone();
    let timeout_receiver = timeout_sender.subscribe();
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

            tokio::spawn(async move {
                timeout_bot(next_player_id, timeout_receiver, timeout_sender, || {
                    RoomMessage {
                        from_user_id: Some(next_player_id),
                        to_user_id: None,
                        msg_type: RoomMessageType::ReplaceCardsBot,
                    }
                })
                .await
            });
        }
        GameState::PlayingHand {
            stack,
            current_scores: _,
        } => {
            // send play event
            for player_id in game.player_ids_in_order() {
                let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = game
                    .get_player_cards(player_id)
                    .map(convert_card_to_player_card);
                sender.send(RoomMessage {
                    from_user_id: None,
                    to_user_id: Some(player_id),
                    msg_type: RoomMessageType::NextPlayerToPlay {
                        current_player_id: next_player_id,
                        current_cards: Some(cards),
                        stack: convert_stack_to_card_player_card(stack),
                    },
                })?;
            }

            tokio::spawn(async move {
                timeout_bot(next_player_id, timeout_receiver, timeout_sender, || {
                    RoomMessage {
                        from_user_id: Some(next_player_id),
                        to_user_id: None,
                        msg_type: RoomMessageType::PlayBot,
                    }
                })
                .await
            });
        }
        any => {
            tracing::warn!("receiving weird event from game after exchange cards: {any:?}");
        }
    }
    Ok(())
}

async fn send_current_state(
    state: &RoomState,
    from_user_id: Uuid,
    sender: &Sender<RoomMessage>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match state {
        RoomState::WaitingForPlayers(ref players_slot) => {
            sender.send(RoomMessage {
                from_user_id: None,
                to_user_id: Some(from_user_id),
                msg_type: RoomMessageType::WaitingForPlayers(*players_slot),
            })?;
        }
        RoomState::Started(ref players, ref game) | RoomState::Done(ref players, ref game) => {
            // send current state
            let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = game
                .get_player_cards(from_user_id)
                .map(convert_card_to_player_card);
            let stack = match &game.state {
                GameState::PlayingHand { ref stack, .. }
                | GameState::ComputeScore { ref stack, .. } => {
                    convert_stack_to_card_player_card(stack)
                }
                _ => [None; PLAYER_NUMBER],
            };
            let state = match &game.state {
                GameState::ExchangeCards { .. } => "EXCHANGE_CARDS",
                GameState::PlayingHand { .. }
                | GameState::EndHand
                | GameState::ComputeScore { .. } => "PLAYING_HAND",
                GameState::End => "END",
            };

            let current_scores = game.current_score_by_id();
            let player_scores = game.player_score_by_id();
            sender.send(RoomMessage {
                from_user_id: None,
                to_user_id: Some(from_user_id),
                msg_type: RoomMessageType::State {
                    mode: Cow::Borrowed(state),
                    player_scores,
                    current_scores,
                    current_cards: Box::new(cards),
                    current_stack: stack,
                    current_hand: game.current_hand,
                    current_player_id: game.current_player_id(),
                    hands: game.hands,
                },
            })?;
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
    tracing::info!("listening room task {id}...");
    while let Ok(msg) = receiver.recv().await {
        tracing::debug!("receiving");
        let Some(from_user_id) = msg.from_user_id else {
            continue;
        };

        match msg.msg_type {
            RoomMessageType::JoinBot => {
                // todo make sure the room creator is the one who send the msg
                let room_guard = room.read().await;

                if let RoomState::WaitingForPlayers(ref players) = room_guard.state {
                    let uuid = Uuid::new_v4();
                    tokio::task::spawn(bot_task(sender.clone(), uuid));
                }
            }
            RoomMessageType::Join => {
                let mut room_guard = room.write().await;
                let is_viewer = room_guard.viewers.iter().any(|p| p == &from_user_id);
                match room_guard.state {
                    RoomState::WaitingForPlayers(ref mut players) => {
                        if players.iter().any(|p| p == &Some(from_user_id)) || is_viewer {
                            sender.send(RoomMessage {
                                from_user_id: None,
                                to_user_id: Some(from_user_id),
                                msg_type: RoomMessageType::PlayerError(GameError::StateError),
                            })?;
                            continue;
                        }

                        let Some(player_slot) = players.iter_mut().find(|p| p.is_none()) else {
                            unreachable!()
                        };

                        *player_slot = Some(from_user_id);
                        sender.send(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::Joined(from_user_id),
                        })?;

                        if players.iter().all(|p| p.is_some()) {
                            let users_guard = users.read().await;
                            let users: [User; PLAYER_NUMBER] = players.map(|player| {
                                let Some(player) = player else { unreachable!() };
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
                            let player_scores = game.player_score_by_id();
                            sender.send(RoomMessage {
                                from_user_id: None,
                                to_user_id: None,
                                msg_type: RoomMessageType::NewHand {
                                    player_ids_in_order,
                                    player_scores,
                                    current_player_id,
                                    current_hand: game.current_hand,
                                    hands: game.hands,
                                },
                            })?;
                            // todo we may need to filter on user that are not bot
                            let timeout_sender = sender.clone();
                            let timeout_receiver = timeout_sender.subscribe();
                            tokio::spawn(async move {
                                timeout_bot(
                                    current_player_id,
                                    timeout_receiver,
                                    timeout_sender,
                                    || RoomMessage {
                                        from_user_id: Some(current_player_id),
                                        to_user_id: None,
                                        msg_type: RoomMessageType::ReplaceCardsBot,
                                    },
                                )
                                .await
                            });
                            // nordine marker
                        }
                    }
                    RoomState::Started(ref users, _) | RoomState::Done(ref users, _) => {
                        if users.iter().any(|u| u.id == from_user_id) {
                            sender.send(RoomMessage {
                                from_user_id: None,
                                to_user_id: Some(from_user_id),
                                msg_type: RoomMessageType::PlayerError(GameError::StateError),
                            })?;
                        } else {
                            room_guard.viewers.insert(from_user_id);
                            sender.send(RoomMessage {
                                from_user_id: None,
                                to_user_id: None,
                                msg_type: RoomMessageType::ViewerJoined(from_user_id),
                            })?;
                        }
                    }
                }
            }
            RoomMessageType::GetCards => {
                let room_guard = room.read().await;
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
                if let RoomState::Started(ref users, ref game) = room_guard.state {
                    let cards: [Option<PlayerCard>; PLAYER_CARD_SIZE] = game
                        .get_player_cards(from_user_id)
                        .map(convert_card_to_player_card);
                    sender.send(RoomMessage {
                        from_user_id: None,
                        to_user_id: Some(from_user_id),
                        msg_type: RoomMessageType::ReceiveCards(cards),
                    })?;
                } else {
                    tracing::debug!("should not happen")
                }
            }

            RoomMessageType::ReplaceCardsBot => {
                tracing::debug!("entering replace card bot");
                let mut room_guard = room.write().await;
                tracing::debug!("no dead lock");
                if let RoomState::Started(ref players, ref mut game) = room_guard.state {
                    if game.current_player_id() == Some(from_user_id)
                    // we don't check if player
                    // is a bot or not, in order to be able to implement timeout later
                    {
                        if let GameState::ExchangeCards { commands: _ } = &game.state {
                            game.play_bot()?;
                            let Some(next_player_id) = game.current_player_id() else {
                                unreachable!()
                            };
                            send_message_after_cards_replaced(game, &sender, next_player_id)
                                .await?;
                        }
                    } else {
                        tracing::error!(
                            "{from_user_id} not current player id {:?}",
                            game.current_player_id()
                        );
                    }
                }
            }

            RoomMessageType::ReplaceCards(player_cards_exchange) => {
                let mut room_guard = room.write().await;
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
                if let RoomState::Started(ref players, ref mut game) = room_guard.state {
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
                                let Some(next_player_id) = game.current_player_id() else {
                                    unreachable!()
                                };
                                send_message_after_cards_replaced(game, &sender, next_player_id)
                                    .await?;
                            }
                        }
                    }
                }
            }
            RoomMessageType::PlayBot => {
                tracing::debug!("receiving playbot message");

                let mut room_guard = room.write().await;
                tracing::debug!("no deadlock...");

                if let RoomState::Started(ref players, ref mut game) = room_guard.state {
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
                                let player_scores = game.player_score_by_id();
                                room_guard.state = RoomState::Done(*players, *game);
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::End { player_scores },
                                })?;
                            }
                        }
                    } else {
                        tracing::error!(
                            "{from_user_id} not current player id {:?}",
                            game.current_player_id()
                        );
                    }
                }
            }
            RoomMessageType::Play(player_card) => {
                let mut room_guard = room.write().await;
                if !is_valid_msg(&room_guard, from_user_id) {
                    continue;
                }
                if let RoomState::Started(ref mut players, ref mut game) = room_guard.state {
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
                                let player_scores = game.player_score_by_id();
                                room_guard.state = RoomState::Done(*players, *game);
                                sender.send(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::End { player_scores },
                                })?;
                            }
                        }
                    }
                }
            }
            RoomMessageType::GetCurrentState => {
                let room_guard = room.read().await;
                send_current_state(&room_guard.state, from_user_id, &sender).await?;
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

    use std::str::FromStr;

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
        assert_eq!(
            RoomMessage {
                from_user_id: Uuid::from_str("96f6b528-4fdc-47ed-8c50-277b13587fc1").ok(),
                to_user_id: None,
                msg_type: crate::room::RoomMessageType::Join
            },
            serde_json::from_str::<RoomMessage>(
                r#"
            {
               "fromUserId": "96f6b528-4fdc-47ed-8c50-277b13587fc1",
               "toUserId" : "96f6b528-4fdc-47ed-8c50-277b13587fcÉ",
               "msgType": "join"
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
