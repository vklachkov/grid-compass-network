#[cfg(not(unix))]
compile_error!("This project is only supported on Unix-like systems.");

mod db;
mod env;
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

use std::{process::ExitCode, sync::Arc, thread};

use anyhow::Context;
use env::Environment;
use log::{error, info};

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
    let env = read_and_log_env()?;
    let conn = db::Database::open(&env.db_path).context("database")?;

    if let Some(addr) = env.web_listen_addr.clone() {
        start_web(addr, Arc::clone(&conn));
    }

    let vfs_root = Arc::new(vfs::VfsDirManager::new(&env.fs_root).context("vfs root")?);
    let server = server::Server::new(conn, vfs_root);

    transport::tcp::serve(&env.listen_addr, server).context("TCP transport")
}

fn read_and_log_env() -> anyhow::Result<Arc<Environment>> {
    let env = Environment::read()?;

    info!(
        target: "server",
        "start server at {}, web at {}, use database from '{}'",
        env.listen_addr,
        env.web_listen_addr.as_deref().unwrap_or("[disabled]"),
        env.db_path,
    );

    Ok(env)
}

fn start_web(web_listen_addr: String, conn: Arc<db::Database>) {
    thread::spawn(move || {
        if let Err(err) = web::serve(web_listen_addr, conn) {
            error!(target: "web", "frontend stopped: {err}");
        }
    });
}
