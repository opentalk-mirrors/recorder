// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) mod cpu;
pub(crate) mod gpu_intel;

pub(crate) static IS_FEASIBLE: AtomicBool = AtomicBool::new(true);
pub fn is_new_recording_feasible() -> bool {
    IS_FEASIBLE.load(Ordering::Relaxed)
}
