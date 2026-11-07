use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serial2::{CharSize, FlowControl, Parity, StopBits};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_path: PathBuf,
    pub filesystem_root: PathBuf,
    pub frontend: Option<Frontend>,
    pub transport: Vec<Transport>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Frontend {
    pub listen_address: SocketAddr,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transport {
    Tcp(TcpConfig),
    Serial(SerialConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct TcpConfig {
    pub listen_address: SocketAddr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SerialConfig {
    pub path: PathBuf,
    pub baud_rate: u32,
    pub char_size: Option<CharSize>,
    pub parity: Option<Parity>,
    pub stop_bits: Option<StopBits>,
    pub flow_control: Option<FlowControl>,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }
}
