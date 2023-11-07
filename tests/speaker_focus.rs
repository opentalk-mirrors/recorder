// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_speaker_focus() {
        EventRunner::run(&[
            Event::JoinUsers(10, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::UpdateConsents(10, true),
            Event::Sleep(Duration::from_secs(2)),
            Event::SpeakerFocusSet(0),
            Event::Sleep(Duration::from_secs(2)),
            Event::SpeakerFocusSet(1),
            Event::Sleep(Duration::from_secs(2)),
            Event::SpeakerFocusUnset,
            Event::Sleep(Duration::from_secs(2)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(2)),
        ])
        .await;
    }
}
