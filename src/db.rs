use std::{error::Error, str::FromStr};

use arraystring::ArrayString;
use sqlx::{sqlite::SqliteRow, Pool, Row, Sqlite};
use uuid::Uuid;

use crate::user::User;

pub async fn find_user_by_id(id: Uuid, pool: &Pool<Sqlite>) -> Result<User, Box<dyn Error>> {
    let row = sqlx::query("select * from users where id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    row_to_user(row)
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

fn row_to_user(row: SqliteRow) -> Result<User, Box<dyn Error>> {
    let name: &str = row.try_get("name")?;
    let id: Uuid = row.try_get("id")?;
    let is_guest: bool = row.try_get("is_guest")?;
    Ok(User {
        id,
        name: ArrayString::from_str(name)?,
        bot: false,
        is_guest,
    })
}
