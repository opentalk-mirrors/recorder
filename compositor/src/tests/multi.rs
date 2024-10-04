// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types_signaling::ParticipantId;

use crate::{testing, Mixer, Speaker, TestSink, TestSource};

fn test_multi(use_video: bool) {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a TestSink
    let mut mixer = Mixer::<TestSource>::create(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        use_video,
        &Default::default(),
    )
    .unwrap();

    for i in 0..10 {
        let name = format!("sink_{i}");
        let sink = TestSink::create(&name, use_video).unwrap();
        mixer.link_sink(&name, sink).unwrap();
    }

    testing::generate_streams(&mut mixer, 0, 3, 3, use_video);
    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();

    mixer.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_multi_audio() {
    test_multi(false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_multi_video() {
    test_multi(true);
}
