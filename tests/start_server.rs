mod common;

#[cfg(test)]
mod tests {

    use crate::common::prelude::*;

    #[tokio::test]
    async fn test_start_server() {
        EventRunner::run(&[
            Event::StartRecording,
            Event::Sleep(Duration::from_secs(2)),
            Event::JoinUsers(10, true, true, false),
            Event::Sleep(Duration::from_secs(2)),
            Event::StopRecording,
            Event::Sleep(Duration::from_secs(10)),
        ])
        .await;
    }
}
