// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::sync::{atomic, Arc};

use env_logger::{Builder, Env, Logger};
use log::{Level, Log, Metadata, Record};

pub(crate) struct PanicLogger {
    parent: Logger,
    error_occurred: Arc<atomic::AtomicBool>,
}

impl PanicLogger {
    pub(crate) fn init(error_occurred: Arc<atomic::AtomicBool>) {
        log::warn!("Initialize logger");
        let env = Env::default();
        let mut builder = Builder::from_env(env);
        let logger = builder.build();
        let max_level = logger.filter();
        let panic_logger = PanicLogger {
            parent: logger,
            error_occurred,
        };

        let result = log::set_boxed_logger(Box::new(panic_logger));
        if result.is_ok() {
            log::set_max_level(max_level);
        }
        result.expect("unable to set panic logger");
    }
}

impl Log for PanicLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.parent.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        self.parent.log(record);
        if record.level() == Level::Error {
            self.error_occurred.store(true, atomic::Ordering::Relaxed);
        }
    }

    fn flush(&self) {
        self.parent.flush();
    }
}
