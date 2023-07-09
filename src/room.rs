use lib_hearts::Game;
use rand::{Rng, RngCore};
use serde::Serialize;

#[derive(Serialize)]
pub struct Room {
    pub id: String,
    #[serde(skip_serializing)]
    state: RoomState,
}

pub enum RoomState {
    WaitingForPlayers([Option<User>; 4]),
    Started([User; 4], Game),
    Done([User; 4], Game),
}

#[derive(Copy, Clone, Serialize)]
pub struct User {
    id: u64,
    name: [char; 12],
    bot: bool,
}

impl Default for User {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let id = rng.next_u64();
        User {
            id,
            name: [
                'G',
                'u',
                'e',
                's',
                't',
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
                rng.gen::<u8>() as char,
            ],
            bot: true,
        }
    }
}

impl User {
    pub fn human(self) -> Self {
        Self { bot: false, ..self }
    }
}

impl Default for Room {
    fn default() -> Self {
        Room {
            id: uuid::Uuid::new_v4().to_string(),
            state: RoomState::WaitingForPlayers([None; 4]),
        }
    }
}
