// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::signaling::media::{MediaSessionState, MediaSessionType};
use types_signaling::ParticipantId;

use crate::{
    testing, EncoderType, MediaDescriptor, Mixer, Speaker, TestSource, WebMParameters, WebMSink,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_webm() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a WebMSink
    let mut mixer = Mixer::<TestSource>::create(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
        &Default::default(),
    )
    .unwrap();

    mixer
        .link_sink(
            "webm_sink",
            WebMSink::create(
                "test",
                &WebMParameters {
                    path: "/tmp/test.webm".to_owned(),
                    encoder_type: EncoderType::CPU,
                },
            )
            .unwrap(),
        )
        .unwrap();

    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();
    // add a stream
    mixer
        .add_stream(
            MediaDescriptor {
                participant_id: ParticipantId::from_u128(0),
                media_type: MediaSessionType::Video,
            },
            "Participant 0".to_owned(),
            Default::default(),
            MediaSessionState::audio_and_video(),
        )
        .unwrap();

    mixer.dot("test_webm", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(3);
}
