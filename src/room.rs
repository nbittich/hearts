use std::{
    borrow::Cow,
    collections::HashSet,
    error::Error,
    fmt::{Debug, Display},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    constants::{
        ABRITRATRY_CHANNEL_CAPACITY, BOT_SLEEP_SECS, COMPUTE_SCORE_DELAY_SECS, DEFAULT_HANDS,
        TIMEOUT_SECS,
    },
    user::{User, UserId, Users},
};
use arraystring::ArrayString;
use async_broadcast::{InactiveReceiver, Receiver, Sender};
use lib_hearts::{
    get_card_by_idx, Card, Game, GameError, GameState, PlayerState, PositionInDeck, TypeCard,
    PLAYER_CARD_SIZE, PLAYER_NUMBER,
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle, time::timeout};
use uuid::Uuid;
pub type CardEmoji = ArrayString<typenum::U4>;
pub type CardStack = [Option<(usize, usize)>; PLAYER_NUMBER];
pub type Rooms = Arc<RwLock<Vec<Arc<RwLock<Room>>>>>;

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
        uuid: Uuid,
    },
    NextPlayerToReplaceCards {
        current_player_id: UserId,
        uuid: Uuid,
    },
    NextPlayerToPlay {
        current_player_id: UserId,
        current_cards: Option<[Option<PlayerCard>; PLAYER_CARD_SIZE]>,
        stack: [Option<PlayerCard>; PLAYER_NUMBER],
        uuid: Uuid,
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
    pub bots: [Option<UserId>; PLAYER_NUMBER],
    #[serde(skip_serializing)]
    pub sender: Option<Sender<RoomMessage>>,
    #[serde(skip_serializing)]
    pub receiver: InactiveReceiver<RoomMessage>,
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
        let (sender, receiver) = async_broadcast::broadcast(ABRITRATRY_CHANNEL_CAPACITY);
        let inactive_receiver = receiver.deactivate();
        let id = Uuid::new_v4();
        let room = Room {
            id,
            bots: [None; PLAYER_NUMBER],
            state: RoomState::WaitingForPlayers([None; PLAYER_NUMBER]),
            viewers: HashSet::with_capacity(5),
            sender: Some(sender),
            receiver: inactive_receiver,
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
    msg_uuid: Uuid,
    mut receiver: Receiver<RoomMessage>,
    sender: Sender<RoomMessage>,
    bot_msg: impl Fn() -> RoomMessage,
) {
    tracing::debug!("spawned timeout for {player_id}");
    let now = Instant::now();
    let mut timeout_act = Duration::from_secs(TIMEOUT_SECS as u64);

    let sub_t = |a: Duration, b: Duration| match a.checked_sub(b) {
        Some(d) => d,
        None => Duration::ZERO,
    };
    while timeout_act != Duration::ZERO {
        tracing::debug!("entering timeout loop with a duration of {timeout_act:?}");

        match timeout(timeout_act, receiver.recv_direct()).await {
            Ok(Ok(rm)) => match rm.msg_type {
                RoomMessageType::NewHand {
                    current_player_id,
                    uuid,
                    ..
                }
                | RoomMessageType::NextPlayerToReplaceCards {
                    current_player_id,
                    uuid,
                }
                | RoomMessageType::NextPlayerToPlay {
                    current_player_id,
                    uuid,
                    ..
                } if msg_uuid != uuid => {
                    tracing::debug!("it's all good mate. {current_player_id}");
                    return;
                }
                RoomMessageType::End { .. } => {
                    tracing::info!("game over. timeout bot");
                    return;
                }
                msg => {
                    timeout_act = sub_t(timeout_act, now.elapsed());
                    tracing::debug!("invalid message: {msg:?}");
                }
            },
            Ok(Err(e)) => {
                timeout_act = sub_t(timeout_act, now.elapsed());
                tracing::debug!("timeout: {e}");
            }
            Err(t) => {
                let msg = bot_msg();
                tracing::error!("{player_id} TIMED OUT. attempt to send {msg:?}");

                receiver.deactivate(); // this is important so we don't broadcast the messages
                                       // below again.
                match sender.broadcast_direct(bot_msg()).await {
                    Ok(res) => {
                        tracing::debug!("message sent => {res:?}");
                    }
                    Err(e) => {
                        tracing::error!("message not sent => {e:?}");
                    }
                }
                match sender
                    .broadcast_direct(RoomMessage {
                        from_user_id: None,
                        to_user_id: Some(player_id),
                        msg_type: RoomMessageType::TimedOut,
                    })
                    .await
                {
                    Ok(res) => {
                        tracing::debug!("message sent => {res:?}");
                    }
                    Err(e) => {
                        tracing::error!("message not sent => {e:?}");
                    }
                }

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
    let Some(current_player_id) = game.current_player_id() else {
        unreachable!()
    };
    let uuid = Uuid::new_v4();

    match &mut game.state {
        GameState::PlayingHand { stack, .. } => {
            sender
                .broadcast_direct(RoomMessage {
                    from_user_id: None,
                    to_user_id: None,
                    msg_type: RoomMessageType::NextPlayerToPlay {
                        current_player_id,
                        current_cards: None,
                        uuid,
                        stack: convert_stack_to_card_player_card(stack),
                    },
                })
                .await?;
            tracing::debug!("LINE 260 => {current_player_id}");
            if game
                .players
                .iter()
                .any(|p| !p.is_bot() && p.get_id() == current_player_id)
            {
                let timeout_sender = sender.clone();
                let timeout_receiver = timeout_sender.new_receiver();
                tokio::spawn(async move {
                    timeout_bot(
                        current_player_id,
                        uuid,
                        timeout_receiver,
                        timeout_sender,
                        || RoomMessage {
                            from_user_id: Some(current_player_id),
                            to_user_id: None,
                            msg_type: RoomMessageType::PlayBot,
                        },
                    )
                    .await
                });
            }
        }
        GameState::ComputeScore { ref stack, .. } => {
            let stack = *stack;

            game.compute_score()?;

            let current_scores = game.current_score_by_id();
            let player_scores = game.player_score_by_id();
            sender
                .broadcast_direct(RoomMessage {
                    from_user_id: None,
                    to_user_id: None,
                    msg_type: RoomMessageType::UpdateStackAndScore {
                        stack: convert_stack_to_card_player_card(&stack),
                        current_scores: Some(current_scores),
                        player_scores,
                    },
                })
                .await?;

            tokio::time::sleep(Duration::from_secs(COMPUTE_SCORE_DELAY_SECS)).await;

            match &game.state {
                GameState::PlayingHand {
                    stack,
                    current_scores,
                } => {
                    let current_player_id = game.current_player_id().ok_or("No current id")?;
                    sender
                        .broadcast_direct(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::NextPlayerToPlay {
                                current_player_id,
                                uuid,
                                current_cards: None,
                                stack: convert_stack_to_card_player_card(stack),
                            },
                        })
                        .await?;

                    tracing::debug!("LINE 306 => {current_player_id}");
                    if game
                        .players
                        .iter()
                        .any(|p| !p.is_bot() && p.get_id() == current_player_id)
                    {
                        // todo we may need to filter on user that are not bot
                        let timeout_sender = sender.clone();
                        let timeout_receiver = timeout_sender.new_receiver();
                        tokio::spawn(async move {
                            timeout_bot(
                                current_player_id,
                                uuid,
                                timeout_receiver,
                                timeout_sender,
                                || RoomMessage {
                                    from_user_id: Some(current_player_id),
                                    to_user_id: None,
                                    msg_type: RoomMessageType::PlayBot,
                                },
                            )
                            .await
                        });
                    }
                }
                GameState::EndHand | GameState::ExchangeCards { commands: _ } => {
                    game.deal_cards()?;
                    let current_player_id = game.current_player_id().ok_or("should not happen")?;

                    let player_ids_in_order = game.player_ids_in_order();
                    let player_scores = game.player_score_by_id();

                    sender
                        .broadcast_direct(RoomMessage {
                            from_user_id: None,
                            to_user_id: None,
                            msg_type: RoomMessageType::NewHand {
                                player_ids_in_order,
                                current_player_id,
                                uuid,
                                player_scores,
                                hands: game.hands,
                                current_hand: game.current_hand,
                            },
                        })
                        .await?;
                    if game
                        .players
                        .iter()
                        .any(|p| !p.is_bot() && p.get_id() == current_player_id)
                    {
                        // todo we may need to filter on user that are not bot
                        let timeout_sender = sender.clone();
                        let timeout_receiver = timeout_sender.new_receiver();
                        tokio::spawn(async move {
                            timeout_bot(
                                current_player_id,
                                uuid,
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
                    }
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
    bot_ids: [Option<UserId>; PLAYER_NUMBER],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing::info!("setup bot task with bots {bot_ids:?}...");
    if bot_ids.iter().all(Option::is_none) {
        tracing::info!("nothing to do.. gonna sleep now");
    }
    let mut receiver = sender.new_receiver();

    while let Ok(msg) = receiver.recv_direct().await {
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
            | RoomMessageType::NextPlayerToReplaceCards {
                current_player_id, ..
            } if bot_ids
                .iter()
                .flatten()
                .find(|b| *b == &current_player_id)
                .is_some() =>
            {
                sender
                    .broadcast_direct(RoomMessage {
                        from_user_id: Some(current_player_id),
                        to_user_id: None,
                        msg_type: RoomMessageType::ReplaceCardsBot,
                    })
                    .await?;
            }

            RoomMessageType::NextPlayerToPlay {
                current_player_id,
                stack,
                ..
            } if bot_ids
                .iter()
                .flatten()
                .find(|b| *b == &current_player_id)
                .is_some() =>
            {
                tracing::debug!("LINE 401 {current_player_id}");
                sender
                    .broadcast_direct(RoomMessage {
                        from_user_id: Some(current_player_id),
                        to_user_id: None,
                        msg_type: RoomMessageType::PlayBot,
                    })
                    .await?;
            }
            RoomMessageType::End { .. } => {
                tracing::info!("bot task say goodbye.");
                return Ok(());
            }

            _ => tracing::debug!("we don't care about {msg:?}"),
        }
        // todo
    }
    tracing::info!("au revoir");
    Ok(())
}

async fn send_message_after_cards_replaced(
    game: &Game,
    sender: &Sender<RoomMessage>,
    next_player_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // todo we may need to filter on user that are not bot
    // be sure to check both cases where sender/receiver are used

    let uuid = Uuid::new_v4();
    match &game.state {
        GameState::ExchangeCards { commands: _ } => {
            // send change cards
            sender
                .broadcast_direct(RoomMessage {
                    from_user_id: None,
                    to_user_id: None,
                    msg_type: RoomMessageType::NextPlayerToReplaceCards {
                        current_player_id: next_player_id,
                        uuid,
                    },
                })
                .await?;
            if game
                .players
                .iter()
                .any(|p| !p.is_bot() && p.get_id() == next_player_id)
            {
                let timeout_sender = sender.clone();
                let timeout_receiver = timeout_sender.new_receiver();
                tokio::spawn(async move {
                    timeout_bot(
                        next_player_id,
                        uuid,
                        timeout_receiver,
                        timeout_sender,
                        || RoomMessage {
                            from_user_id: Some(next_player_id),
                            to_user_id: None,
                            msg_type: RoomMessageType::ReplaceCardsBot,
                        },
                    )
                    .await
                });
            }
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
                sender
                    .broadcast_direct(RoomMessage {
                        from_user_id: None,
                        to_user_id: Some(player_id),
                        msg_type: RoomMessageType::NextPlayerToPlay {
                            current_player_id: next_player_id,
                            current_cards: Some(cards),
                            uuid,
                            stack: convert_stack_to_card_player_card(stack),
                        },
                    })
                    .await?;
            }

            tracing::debug!(" LINE 471 => {next_player_id}");
            if game
                .players
                .iter()
                .any(|p| !p.is_bot() && p.get_id() == next_player_id)
            {
                let timeout_sender = sender.clone();
                let timeout_receiver = timeout_sender.new_receiver();
                tokio::spawn(async move {
                    timeout_bot(
                        next_player_id,
                        uuid,
                        timeout_receiver,
                        timeout_sender,
                        || RoomMessage {
                            from_user_id: Some(next_player_id),
                            to_user_id: None,
                            msg_type: RoomMessageType::PlayBot,
                        },
                    )
                    .await
                });
            }
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
            sender
                .broadcast_direct(RoomMessage {
                    from_user_id: None,
                    to_user_id: Some(from_user_id),
                    msg_type: RoomMessageType::WaitingForPlayers(*players_slot),
                })
                .await?;
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
            sender
                .broadcast_direct(RoomMessage {
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
                })
                .await?;
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
        let mut room_guard = room.write().await;
        let Some(sender) = room_guard.sender.take() else {
            panic!("no sender")
        };
        let receiver = sender.new_receiver();
        (sender, receiver)
    };
    let room = room.clone();
    tracing::info!("listening room task {id}...");
    while let Ok(msg) = receiver.recv_direct().await {
        tracing::info!(
            "receiver count {}, inactive receiver count {}, sender count {}, message in the channel {}",
            sender.receiver_count(),
            sender.inactive_receiver_count(),
            sender.sender_count(),
            sender.len()

        );

        let Some(from_user_id) = msg.from_user_id else {
            continue;
        };

        match msg.msg_type {
            RoomMessageType::JoinBot => {
                // todo make sure the room creator is the one who send the msg
                let mut room_guard = room.write().await;

                if let RoomState::WaitingForPlayers(ref players) = &room_guard.state {
                    let uuid = Uuid::new_v4();
                    let Some(bot_seat) = room_guard.bots.iter_mut().find(|b| b.is_none()) else {
                        unreachable!("no seats for bot")
                    };
                    *bot_seat = Some(uuid);

                    sender
                        .broadcast_direct(RoomMessage {
                            from_user_id: Some(uuid),
                            to_user_id: None,
                            msg_type: RoomMessageType::Join,
                        })
                        .await?;
                }
            }
            RoomMessageType::Join => {
                let mut room_guard = room.write().await;
                let is_viewer = room_guard.viewers.iter().any(|p| p == &from_user_id);
                let bots = room_guard.bots;

                match room_guard.state {
                    RoomState::WaitingForPlayers(ref mut players) => {
                        if players.iter().any(|p| p == &Some(from_user_id)) || is_viewer {
                            sender
                                .broadcast_direct(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: Some(from_user_id),
                                    msg_type: RoomMessageType::PlayerError(GameError::StateError),
                                })
                                .await?;
                            continue;
                        }

                        let Some(player_slot) = players.iter_mut().find(|p| p.is_none()) else {
                            unreachable!()
                        };

                        *player_slot = Some(from_user_id);
                        sender
                            .broadcast_direct(RoomMessage {
                                from_user_id: None,
                                to_user_id: None,
                                msg_type: RoomMessageType::Joined(from_user_id),
                            })
                            .await?;

                        if players.iter().all(|p| p.is_some()) {
                            let sender_bot = sender.clone();
                            tokio::spawn(async move { bot_task(sender_bot, bots).await });
                            tokio::time::sleep(Duration::from_millis(500)).await;
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
                            let uuid = Uuid::new_v4();
                            sender
                                .broadcast_direct(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::NewHand {
                                        player_ids_in_order,
                                        player_scores,
                                        uuid,
                                        current_player_id,
                                        current_hand: game.current_hand,
                                        hands: game.hands,
                                    },
                                })
                                .await?;
                            if game
                                .players
                                .iter()
                                .any(|p| !p.is_bot() && p.get_id() == current_player_id)
                            {
                                // todo we may need to filter on user that are not bot
                                let timeout_sender = sender.clone();
                                let timeout_receiver = timeout_sender.new_receiver();
                                tokio::spawn(async move {
                                    timeout_bot(
                                        current_player_id,
                                        uuid,
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
                            }
                        }
                    }
                    RoomState::Started(ref users, _) | RoomState::Done(ref users, _) => {
                        if users.iter().any(|u| u.id == from_user_id) {
                            sender
                                .broadcast_direct(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: Some(from_user_id),
                                    msg_type: RoomMessageType::PlayerError(GameError::StateError),
                                })
                                .await?;
                        } else {
                            room_guard.viewers.insert(from_user_id);
                            sender
                                .broadcast_direct(RoomMessage {
                                    from_user_id: None,
                                    to_user_id: None,
                                    msg_type: RoomMessageType::ViewerJoined(from_user_id),
                                })
                                .await?;
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
                    sender
                        .broadcast_direct(RoomMessage {
                            from_user_id: None,
                            to_user_id: Some(from_user_id),
                            msg_type: RoomMessageType::ReceiveCards(cards),
                        })
                        .await?;
                } else {
                    tracing::debug!("should not happen")
                }
            }

            RoomMessageType::ReplaceCardsBot => {
                tracing::debug!("entering replace card bot");
                tokio::time::sleep(Duration::from_secs(BOT_SLEEP_SECS)).await; // give some delay

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
                            tracing::debug!(
                                "after exchange cards, send message for next {next_player_id}"
                            );
                            send_message_after_cards_replaced(game, &sender, next_player_id)
                                .await?;
                        } else {
                            tracing::debug!("state not exchange cards {:?}", game.state);
                        }
                    } else {
                        tracing::error!(
                            "REPLACE CARD BOT ERR:{from_user_id} not current player id {:?}",
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
                                sender
                                    .broadcast_direct(RoomMessage {
                                        from_user_id: None,
                                        to_user_id: Some(from_user_id),
                                        msg_type: RoomMessageType::PlayerError(game_error),
                                    })
                                    .await?;
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

                tokio::time::sleep(Duration::from_secs(BOT_SLEEP_SECS)).await; // give some delay
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
                                sender
                                    .broadcast_direct(RoomMessage {
                                        from_user_id: None,
                                        to_user_id: None,
                                        msg_type: RoomMessageType::End { player_scores },
                                    })
                                    .await?;
                            }
                        }
                    } else {
                        tracing::error!(
                            "PLAYER BOT ERR: {from_user_id} not current player id {:?}",
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
                                sender
                                    .broadcast_direct(RoomMessage {
                                        from_user_id: None,
                                        to_user_id: Some(from_user_id),
                                        msg_type: RoomMessageType::PlayerError(game_error),
                                    })
                                    .await?;
                            } else if send_message_after_played(game, &sender).await? {
                                // game is done, update state
                                let player_scores = game.player_score_by_id();
                                room_guard.state = RoomState::Done(*players, *game);
                                sender
                                    .broadcast_direct(RoomMessage {
                                        from_user_id: None,
                                        to_user_id: None,
                                        msg_type: RoomMessageType::End { player_scores },
                                    })
                                    .await?;
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
