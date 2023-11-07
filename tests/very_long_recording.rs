// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    #[ignore = "takes one hour to complete"]
    /// This tests starts a recording a for an hour and join a new participant every minute.
    /// The concurrent users are limited.
    async fn test_very_long_recording() {
        const MAX_USERS_CONCURRENTLY: usize = 5;

        let mut events = vec![
            Event::JoinUser(0),
            Event::UpdateMedia(0, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::UpdateConsent(0, true),
        ];

        for index in 1..=HOUR_IN_MINUTE as usize {
            if index > MAX_USERS_CONCURRENTLY {
                events.push(Event::LeftUser(index - MAX_USERS_CONCURRENTLY));
            }
            events.push(Event::Sleep(Duration::from_secs(5)));
            events.push(Event::JoinUser(index));
            events.push(Event::Sleep(Duration::from_secs(5)));
            events.push(Event::UpdateMedia(index, true, true, false));
            events.push(Event::Sleep(Duration::from_secs(5)));
            events.push(Event::UpdateConsent(index, true));
            events.push(Event::Sleep(Duration::from_secs(45)));
            // The total amount of all sleep durations must be 60 seconds.
        }

        events.push(Event::StopRecording);

        EventRunner::run(events.as_slice()).await;
    }
}
