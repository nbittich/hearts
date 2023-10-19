create table if not exists users {
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nickname VARCHAR(30),
    date_created timestamp NOT NULL,
    date_modified timestamp
};

create table if not exists rooms {
    id INTEGER PRIMARY KEY AUTOINCREMENT
};

create table if not exists player_room {
    player_id INTEGER,
    room_id INTEGER,
    FOREIGN KEY (player_id) REFERENCES users(id),
    FOREIGN KEY (room_id) REFERENCES rooms(id),
};
