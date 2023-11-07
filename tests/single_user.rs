// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_single_user() {
        EventRunner::run(&[
            Event::StartRecording,
            Event::JoinUser(1),
            Event::UpdateConsent(1, true),
            Event::UpdateMedia(1, true, false, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::UpdateMedia(1, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(2)),
        ])
        .await;
    }
}
