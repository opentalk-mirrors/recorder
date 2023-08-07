mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_multiple_user() {
        EventRunner::run(&[
            Event::JoinUsers(10, true, true, false),
            Event::Sleep(Duration::from_secs(5)),
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(1)),
            Event::UpdateConsents(10, true),
            Event::Sleep(Duration::from_secs(5)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(10)),
        ])
        .await;
    }
}
