create table if not exists users {
    id BLOB PRIMARY KEY,
    nickname TEXT,
    date_created INTEGER NOT NULL,
    date_modified INTEGER
};

create table if not exists rooms {
    id BLOB PRIMARY KEY,
    game_state TEXT -- json 
};

-- keep this to easily show list of rooms for a given user
create table if not exists player_room {
    player_id BLOB,
    room_id BLOB,
    FOREIGN KEY (player_id) REFERENCES users(id),
    FOREIGN KEY (room_id) REFERENCES rooms(id)
};
