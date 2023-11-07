// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod blinder_sink;
mod dash_sink;
mod display_sink;
mod fake_sink;
mod matroska_sink;
mod mp4_sink;
mod multi_sink;
mod rtmp_sink;
mod test_sink;

pub use blinder_sink::*;
pub use dash_sink::*;
pub use display_sink::*;
pub use fake_sink::*;
pub use matroska_sink::*;
pub use mp4_sink::*;
pub use multi_sink::*;
pub use rtmp_sink::*;
pub use test_sink::*;
