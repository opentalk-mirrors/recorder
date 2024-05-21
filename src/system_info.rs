// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use sysinfo::{self, RefreshKind, System};

static IS_FEASIBLE: AtomicBool = AtomicBool::new(true);
pub fn is_new_recording_feasible() -> bool {
    IS_FEASIBLE.load(Ordering::Relaxed)
}

pub fn cpu_usage_poll(hysteresis: u8) {
    const INTERVAL: Duration = Duration::from_secs(1u64);
    let mut rti = RuntimeInformation::new(hysteresis);
    loop {
        rti.setup_cpu_poll();
        std::thread::sleep(INTERVAL);
    }
}

struct RuntimeInformation {
    hysteresis: u8,
    last_cpu_usage: u32,
    system: System,
}

impl RuntimeInformation {
    pub fn new(hysteresis: u8) -> Self {
        Self {
            hysteresis,
            system: System::new_with_specifics(RefreshKind::everything()),
            last_cpu_usage: 0,
        }
    }

    pub fn setup_cpu_poll(&mut self) {
        const MAX_CPU_USAGE: u32 = 80;
        // Enforce an update happened
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        self.system.refresh_cpu();

        let mut cpu_usage = 0;
        for cpu in self.system.cpus() {
            cpu_usage += cpu.cpu_usage() as u32;
        }

        cpu_usage = cpu_usage.saturating_div(self.system.cpus().len() as u32);
        self.last_cpu_usage = cpu_usage;

        IS_FEASIBLE.store(
            self.last_cpu_usage <= (MAX_CPU_USAGE + u32::from(self.hysteresis)),
            Ordering::Relaxed,
        );
    }
}
