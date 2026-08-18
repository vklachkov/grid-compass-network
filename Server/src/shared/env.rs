use anyhow::{Context, Result};

pub fn read_env(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("environment variable {name} is not set"))
}
