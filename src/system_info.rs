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

pub fn cpu_usage_poll(cutoff: u8) {
    const INTERVAL: Duration = Duration::from_secs(1u64);
    let mut rti = RuntimeInformation::new(cutoff);
    loop {
        rti.setup_cpu_poll();
        std::thread::sleep(INTERVAL);
    }
}

struct RuntimeInformation {
    cutoff: u8,
    last_cpu_usage: u32,
    system: System,
}

impl RuntimeInformation {
    pub fn new(cutoff: u8) -> Self {
        Self {
            cutoff,
            system: System::new_with_specifics(RefreshKind::everything()),
            last_cpu_usage: 0,
        }
    }

    pub fn setup_cpu_poll(&mut self) {
        // Enforce an update happened
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        self.system.refresh_cpu_all();

        let max_cpu_usage: u32 = u32::from(self.cutoff) * self.system.cpus().len() as u32;

        let mut cpu_usage = 0;
        for cpu in self.system.cpus() {
            cpu_usage += cpu.cpu_usage() as u32;
        }

        self.last_cpu_usage = cpu_usage;

        IS_FEASIBLE.store(self.last_cpu_usage <= max_cpu_usage, Ordering::Relaxed);
    }
}
