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
            Event::Sleep(Duration::from_secs(1)),
            // FIXME: Audio isn't working here
            Event::UpdateMedia(1, true, true, false),
            Event::Sleep(Duration::from_secs(5)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(10)),
        ])
        .await;
    }
}
