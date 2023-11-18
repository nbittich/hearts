create table if not exists rooms (
    id BLOB PRIMARY KEY NOT NULL,
    state TEXT NOT NULL,
    bots TEXT NOT NULL

);

create table if not exists room_viewers (
  room_id BLOB NOT NULL,
  user_id BLOB NOT NULL,
  FOREIGN KEY (room_id)
      REFERENCES rooms (id),
  FOREIGN KEY (user_id)
      REFERENCES users(id)
);
