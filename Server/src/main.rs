#[cfg(not(unix))]
compile_error!("This project is only supported on Unix-like systems.");

mod config;
mod db;
mod frame;
mod logger;
mod server;
mod shared;
mod transport;
mod vfs;
mod vipc;
mod web;

#[cfg(test)]
mod tests;

use std::{
    env, io,
    net::SocketAddr,
    path::PathBuf,
    process::{self, ExitCode},
    sync::Arc,
    thread,
};

use anyhow::{Context, bail};
use log::{debug, error};

use crate::config::{Config, SerialConfig, Transport};

fn main() -> ExitCode {
    logger::init();

    match run() {
        Ok(()) => {
            ExitCode::SUCCESS //
        }
        Err(err) => {
            error!(target: "server", "fatal error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let config = read_config()?;

    let db_conn = db::Database::open(&config.database_path).context("database")?;
    let vfs_root = Arc::new(vfs::VfsDirManager::new(&config.filesystem_root).context("vfs root")?);

    let server = server::Server::new(Arc::clone(&db_conn), vfs_root);

    if let Some(frontend) = config.frontend {
        start_web(frontend.listen_address, db_conn);
    }

    for transport in config.transport {
        start_transport(transport, server.clone());
    }

    // TODO: Handle SIGTERM, SIGINT gracefully.
    loop {
        thread::park();
    }
}

fn read_config() -> anyhow::Result<Config> {
    let Some(path) = env::args_os().nth(1).map(PathBuf::from) else {
        bail!("expected config file path as first argument")
    };

    let config =
        Config::read(&path).with_context(|| format!("read config from {}", path.display()))?;

    debug!("Use configuration {config:#?}");

    Ok(config)
}

fn start_transport(transport: Transport, server: server::Server) {
    thread::spawn(move || {
        let result = match &transport {
            Transport::Tcp(config) => {
                transport::tcp::serve(server, config.listen_address) //
            }
            Transport::Serial(config) => {
                transport::serial::serve(server, &config.path, |settings| {
                    apply_serial_settings(config, settings)
                })
            }
        };

        if let Err(err) = result {
            error!(target: "server", "transport {transport:?} crashed: {err}");
            process::abort();
        }
    });
}

fn apply_serial_settings(
    config: &SerialConfig,
    mut settings: serial2::Settings,
) -> io::Result<serial2::Settings> {
    settings.set_raw();

    settings.set_baud_rate(config.baud_rate)?;

    if let Some(value) = config.char_size {
        settings.set_char_size(value);
    }

    if let Some(value) = config.parity {
        settings.set_parity(value);
    }

    if let Some(value) = config.stop_bits {
        settings.set_stop_bits(value);
    }

    if let Some(value) = config.flow_control {
        settings.set_flow_control(value);
    }

    Ok(settings)
}

fn start_web(web_listen_addr: SocketAddr, conn: Arc<db::Database>) {
    thread::spawn(move || {
        if let Err(err) = web::serve(web_listen_addr.to_string(), conn) {
            error!(target: "web", "frontend stopped: {err}");
        }
    });
}
