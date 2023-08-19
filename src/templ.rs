use std::{error::Error, sync::OnceLock};

use minijinja::Environment;
use serde::Serialize;

static ENGINE: OnceLock<Environment<'static>> = OnceLock::new();

pub static INDEX_PAGE: &str = "index.html";
pub static ROOM_PAGE: &str = "room.html";
pub static BASE_LAYOUT: &str = "base.html";

pub fn get_template<S: Serialize>(tpl: &str, ctx: S) -> Result<String, Box<dyn Error>> {
    let engine = {
        if ENGINE.get().is_none() {
            ENGINE.set(init_engine()?).unwrap_or_default();
        }
        ENGINE.get().ok_or("could not extract engine from cell")
    }?;
    let template = engine.get_template(tpl)?;
    let res = template.render(ctx)?;
    Ok(res)
}

fn init_engine() -> Result<Environment<'static>, Box<dyn Error>> {
    let mut env = Environment::new();
    env.add_template(BASE_LAYOUT, include_str!("templates/base.html"))?;
    env.add_template(INDEX_PAGE, include_str!("templates/index.html"))?;
    env.add_template(ROOM_PAGE, include_str!("templates/room.html"))?;
    Ok(env)
}
