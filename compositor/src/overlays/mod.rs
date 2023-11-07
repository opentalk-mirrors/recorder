// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlays module.
mod clock_overlay;
mod padding_overlay;
mod talk_overlay;
mod text_overlay;

pub use clock_overlay::*;
pub use padding_overlay::*;
pub use talk_overlay::*;
pub use text_overlay::*;
