mod db;
mod gridlink;
mod gridserver;
mod logger;
mod services;
mod shared;
mod web;

use std::{process::ExitCode, thread};

use log::error;

fn main() -> ExitCode {
    logger::init();

    // The frontend is not what the server is for: it runs beside the GRiD
    // listener and its failure to bind must not take the listener down with it.
    thread::spawn(|| {
        if let Err(err) = web::serve() {
            error!(target: "web", "frontend stopped: {err}");
        }
    });

    match gridserver::serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(target: "server", "fatal error: {err}");
            ExitCode::FAILURE
        }
    }
}
