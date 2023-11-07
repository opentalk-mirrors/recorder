// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_join_left() {
        EventRunner::run(&[
            Event::JoinUser(0),
            Event::UpdateMedia(0, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::UpdateConsent(0, true),
            Event::Sleep(Duration::from_secs(2)),
            Event::JoinUser(1),
            Event::UpdateMedia(1, true, true, false),
            Event::UpdateConsent(1, true),
            Event::Sleep(Duration::from_secs(2)),
            Event::LeftUser(0),
            Event::Sleep(Duration::from_secs(2)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::LeftUser(1),
            Event::Sleep(Duration::from_secs(2)),
        ])
        .await;
    }
}
