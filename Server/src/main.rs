mod db;
mod gridlink;
mod gridserver;
mod logger;
mod services;
mod shared;

use std::process::ExitCode;

use log::error;

fn main() -> ExitCode {
    logger::init();

    match gridserver::serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(target: "server", "fatal error: {err}");
            ExitCode::FAILURE
        }
    }
}
