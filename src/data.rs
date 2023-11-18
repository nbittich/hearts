use std::{borrow::Cow, collections::HashSet, error::Error, fmt::Display, sync::Arc};

use arraystring::ArrayString;
use async_broadcast::{InactiveReceiver, Sender};
use dashmap::DashMap;
use lib_hearts::{
    Game, GameError, PlayerState, PositionInDeck, TypeCard, PLAYER_CARD_SIZE, PLAYER_NUMBER,
};
use serde_derive::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use tokio::{sync::RwLock, task::JoinHandle};
use uuid::Uuid;

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
    StartHand {
        current_player_id: UserId,
        uuid: Uuid,
    },
    Join,
    TimedOut,
    JoinBot,
    Joined(UserId),
    ViewerJoined(UserId),
    GetCards,
    ReceiveCards([Option<PlayerCard>; PLAYER_CARD_SIZE]),
    ReplaceCards([PlayerCard; lib_hearts::NUMBER_REPLACEABLE_CARDS]),
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
pub type CardEmoji = ArrayString<typenum::U4>;
pub type CardStack = [Option<(usize, usize)>; PLAYER_NUMBER];
pub type Rooms = Arc<DashMap<Uuid, Arc<RwLock<Room>>>>;

#[derive(Serialize)]
pub struct Room {
    pub id: Uuid,
    pub state: RoomState,
    pub viewers: HashSet<UserId>,
    pub bots: [Option<UserId>; PLAYER_NUMBER],
    #[serde(skip_serializing)]
    pub sender: Option<Sender<RoomMessage>>,
    #[serde(skip_serializing)]
    pub receiver: InactiveReceiver<RoomMessage>,
    #[serde(skip_serializing)]
    pub task: Option<JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    #[serde(skip_serializing)]
    pub pool: Pool<Sqlite>,
}

#[derive(Serialize, Deserialize)]
pub enum RoomState {
    WaitingForPlayers([Option<UserId>; PLAYER_NUMBER]),
    Started([User; PLAYER_NUMBER], Game),
    Done([User; PLAYER_NUMBER], Game),
}

pub struct DbRoom {
    pub id: Uuid,
    pub state: RoomState,
    pub bots: [Option<UserId>; lib_hearts::PLAYER_NUMBER],
    pub viewers: HashSet<UserId>,
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
pub type UserId = Uuid;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub is_guest: bool,
    pub name: ArrayString<typenum::U12>,
    pub bot: bool,
}
