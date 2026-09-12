use std::{env, sync::Arc};

use anyhow::Context;

#[derive(Clone)]
pub struct Environment {
    pub listen_addr: String,
    pub db_path: String,
    pub fs_root: String,
    pub web_listen_addr: Option<String>,
}

impl Environment {
    pub fn read() -> anyhow::Result<Arc<Self>> {
        let this = Self {
            listen_addr: required("LISTEN_ADDR")?,
            db_path: required("DB_PATH")?,
            fs_root: required("FS_ROOT")?,
            web_listen_addr: optional("WEB_LISTEN_ADDR"),
        };

        Ok(Arc::new(this))
    }
}

fn required(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!("environment variable {name} is not set"))
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok()
}
