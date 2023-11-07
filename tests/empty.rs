// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_empty() {
        EventRunner::run(&[
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(10)),
            Event::JoinUsers(1, false, false, false),
            Event::Sleep(Duration::from_secs(10)),
            Event::StopRecording,
        ])
        .await;
    }
}
