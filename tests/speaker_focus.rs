mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_speaker_focus() {
        EventRunner::run(&[
            Event::JoinUsers(10, true, true, false),
            Event::Sleep(Duration::from_secs(5)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(1)),
            Event::UpdateConsents(10, true),
            Event::Sleep(Duration::from_secs(5)),
            Event::SpeakerFocusSet(0),
            Event::Sleep(Duration::from_secs(5)),
            Event::SpeakerFocusSet(1),
            Event::Sleep(Duration::from_secs(5)),
            Event::SpeakerFocusUnset,
            Event::Sleep(Duration::from_secs(5)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(10)),
        ])
        .await;
    }
}
