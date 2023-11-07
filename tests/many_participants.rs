// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    const AMOUNT_OF_CONCURRENT_PARTICIPANTS: usize = 100;

    #[tokio::test]
    #[ignore = "this tests takes very long"]
    async fn test_many_participants_slow() {
        let mut events = vec![
            Event::JoinUser(0),
            Event::UpdateMedia(0, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::UpdateConsent(0, true),
        ];

        for index in 1..AMOUNT_OF_CONCURRENT_PARTICIPANTS {
            events.push(Event::JoinUser(index));
            events.push(Event::UpdateMedia(index, true, true, false));
            events.push(Event::Sleep(Duration::from_secs(2)));
            events.push(Event::UpdateConsent(index, true));
            events.push(Event::Sleep(Duration::from_secs(2)));
        }

        events.push(Event::StopRecording);

        EventRunner::run(events.as_slice()).await;
    }

    #[tokio::test]
    #[ignore = "this tests takes very long"]
    async fn test_many_participants_fast() {
        EventRunner::run(&[
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::JoinUsers(AMOUNT_OF_CONCURRENT_PARTICIPANTS, true, true, false),
            Event::UpdateConsents(AMOUNT_OF_CONCURRENT_PARTICIPANTS, true),
            Event::Sleep(Duration::from_secs(5)),
            Event::StopRecording,
        ])
        .await;
    }
}
