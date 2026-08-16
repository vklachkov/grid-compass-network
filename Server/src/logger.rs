use std::{
    io::{IsTerminal, Write},
    sync::OnceLock,
    time::Instant,
};

use log::{Level, LevelFilter, Log, Metadata, Record};

static STARTED_AT: OnceLock<Instant> = OnceLock::new();

struct Logger {
    filter: env_filter::Filter,
    color: bool,
}

pub fn init() {
    STARTED_AT.get_or_init(Instant::now);

    let mut builder = env_filter::Builder::new();
    match std::env::var("RUST_LOG") {
        Ok(spec) => builder.parse(&spec),
        Err(_) => builder.filter_level(LevelFilter::Info),
    };

    let filter = builder.build();
    let color = std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();

    log::set_max_level(filter.filter());
    let _ = log::set_boxed_logger(Box::new(Logger { filter, color }));
}

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

fn level_color(level: Level) -> &'static str {
    match level {
        Level::Error => "\x1b[31m",
        Level::Warn => "\x1b[33m",
        Level::Info => "\x1b[32m",
        Level::Debug => "\x1b[36m",
        Level::Trace => "\x1b[35m",
    }
}

fn elapsed() -> (u64, u32) {
    let elapsed = STARTED_AT.get_or_init(Instant::now).elapsed();
    (elapsed.as_secs(), elapsed.subsec_millis())
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if !self.filter.matches(record) {
            return;
        }

        let (secs, millis) = elapsed();

        let line = if self.color {
            format!(
                "{DIM}[{secs:>6}.{millis:03}]{RESET} {}{:5}{RESET} {DIM}{}:{RESET} {}",
                level_color(record.level()),
                record.level(),
                record.target(),
                record.args(),
            )
        } else {
            format!(
                "[{secs:>6}.{millis:03}] {:5} {}: {}",
                record.level(),
                record.target(),
                record.args(),
            )
        };

        let _ = writeln!(std::io::stdout(), "{line}");
    }

    fn flush(&self) {
        let _ = std::io::stdout().flush();
    }
}
