use chrono::Local;
use log::{LevelFilter, SetLoggerError};
use log::{Metadata, Record};

use crate::PROGRESS_BAR;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!(
                "{} [{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%.6f"),
                record.level(),
                record.target(),
                record.args()
            );

            if PROGRESS_BAR.is_hidden() {
                eprintln!("{}", msg);
            } else {
                PROGRESS_BAR.println(msg);
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

pub fn init_logging(filter: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)?;
    log::set_max_level(filter);

    Ok(())
}
