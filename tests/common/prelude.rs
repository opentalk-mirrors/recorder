pub(crate) use crate::common::{Event, EventRunner};
pub(crate) use std::time::Duration;

pub(crate) const MINUTE_IN_SECS: u64 = 60;
pub(crate) const HOUR_IN_MINUTE: u64 = 60;
pub(crate) const HOUR_IN_SECS: u64 = HOUR_IN_MINUTE * MINUTE_IN_SECS;
