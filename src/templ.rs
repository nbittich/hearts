use std::{cell::OnceCell, error::Error};

use minijinja::Environment;
use serde::Serialize;

const ENGINE: OnceCell<Environment> = OnceCell::new();

pub const INDEX_PAGE: &str = "index.html";
pub const BASE_LAYOUT: &str = "base.html";

pub fn get_template<S: Serialize>(tpl: &str, ctx: S) -> Result<String, Box<dyn Error>> {
    let engine = ENGINE;
    let engine = {
        if engine.get().is_none() {
            engine.set(init_engine()?).unwrap_or_default();
        }
        engine.get().ok_or("could not extract engine from cell")
    }?;
    let template = engine.get_template(tpl)?;
    let res = template.render(ctx)?;
    Ok(res)
}

fn init_engine() -> Result<Environment<'static>, Box<dyn Error>> {
    let mut env = Environment::new();
    env.add_template(BASE_LAYOUT, include_str!("templates/base.html"))?;
    env.add_template(INDEX_PAGE, include_str!("templates/index.html"))?;
    Ok(env)
}
