// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use gst::glib::MainLoop;

use crate::{
    recorder::{Recorder, RecordingSession},
    rmq::StartRecording,
};

fn init() -> MainLoop {
    let _ = env_logger::try_init();

    gst::init().expect("gstreamer init failed");

    MainLoop::new(None, false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn basic_test() {
    let gst_loop = init();
    let context = Recorder {
        settings,
        http_client,
    };
    let session = RecordingSession::create(
        &context,
        StartRecording {
            room: "0".to_string(),
            breakout: None,
        },
    )
    .await
    .expect("session init failed");
}
