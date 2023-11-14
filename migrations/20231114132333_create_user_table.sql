create table if not exists users (
    id BLOB PRIMARY KEY,
    name TEXT,
    is_guest BOOLEAN NOT NULL CHECK(is_guest IN (0,1))
);


