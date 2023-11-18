use std::{collections::HashSet, error::Error, str::FromStr};

use arraystring::ArrayString;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use crate::data::{DbRoom, Room, User};

pub async fn find_user_by_id(id: Uuid, pool: &Pool<Sqlite>) -> Result<User, Box<dyn Error>> {
    let row = sqlx::query!("select id, name, is_guest from users where id = ?", id)
        .fetch_one(pool)
        .await?;
    row_to_user(row.id, row.name, row.is_guest)
}

pub async fn upsert_user(user: User, pool: &Pool<Sqlite>) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.acquire().await?;
    let name = user.name.to_string();
    let is_guest = user.is_guest;
    let id = user.id;
    let _ = sqlx::query!(
        r#"
        INSERT into users (id, name, is_guest)
        VALUES (?1, ?2, ?3)
        ON CONFLICT DO UPDATE SET name=?2, is_guest=?3;
    "#,
        id,
        name,
        is_guest
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn row_to_user(id: Vec<u8>, name: String, is_guest: bool) -> Result<User, Box<dyn Error>> {
    Ok(User {
        id: Uuid::from_slice(&id[..])?,
        name: ArrayString::from_str(&name)?,
        bot: false,
        is_guest,
    })
}

pub async fn find_room_by_id(id: Uuid, pool: &Pool<Sqlite>) -> Result<DbRoom, Box<dyn Error>> {
    let row = sqlx::query!("select id, state, bots from rooms where id = ?", id)
        .fetch_one(pool)
        .await?;
    let id = Uuid::from_slice(&row.id[..])?;

    let row_viewers = sqlx::query!("select user_id from room_viewers where room_id = ?", id)
        .fetch_all(pool)
        .await?;

    row_to_db_room(
        id,
        row.state,
        row.bots,
        row_viewers.into_iter().map(|v| v.user_id).collect(),
    )
}

pub async fn find_all_rooms(id: Uuid, pool: &Pool<Sqlite>) -> Result<Vec<DbRoom>, Box<dyn Error>> {
    let rows = sqlx::query!("select id, state, bots from rooms")
        .fetch_all(pool)
        .await?;
    let mut rooms = vec![];

    for row in rows {
        let id = Uuid::from_slice(&row.id[..])?;

        let row_viewers = sqlx::query!("select user_id from room_viewers where room_id = ?", id)
            .fetch_all(pool)
            .await?;

        let room = row_to_db_room(
            id,
            row.state,
            row.bots,
            row_viewers.into_iter().map(|v| v.user_id).collect(),
        )?;
        rooms.push(room);
    }
    Ok(rooms)
}

fn row_to_db_room(
    id: Uuid,
    state: String,
    bots: String,
    viewers: Vec<Vec<u8>>,
) -> Result<DbRoom, Box<dyn Error>> {
    Ok(DbRoom {
        id,
        state: serde_json::from_str(&state)?,
        bots: serde_json::from_str(&bots)?,
        viewers: viewers
            .into_iter()
            .map(|v| Uuid::from_slice(&v[..]))
            .collect::<Result<HashSet<_>, _>>()?,
    })
}

pub async fn upsert_room(room: &Room, pool: &Pool<Sqlite>) -> Result<(), Box<dyn Error>> {
    let mut conn = pool.acquire().await?;
    let state = serde_json::to_string(&room.state)?;
    let bots = serde_json::to_string(&room.bots)?;
    let id = room.id;
    let _ = sqlx::query!(
        r#"
        INSERT into rooms(id, state, bots)
        VALUES (?1, ?2, ?3)
        ON CONFLICT DO UPDATE SET state=?2, bots=?3;
    "#,
        id,
        state,
        bots
    )
    .execute(&mut *conn)
    .await?;

    for viewers in room.viewers.iter() {
        let _ = sqlx::query!(
            r#"INSERT into room_viewers (room_id,user_id) 
           VALUES (?1,?2)
           ON CONFLICT DO NOTHING;"#,
            id,
            viewers
        )
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}
