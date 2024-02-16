// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::core::ParticipantId;
use types::signaling::media::{MediaSessionState, MediaSessionType};

use crate::{testing, MatroskaSink, MediaDescriptor, Mixer, Speaker, TestSource};

#[test]
fn test_matroska() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();

    mixer
        .link_sink(
            "matroska_sink",
            MatroskaSink::create("test", &Default::default()).unwrap(),
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

    mixer.dot("test_matroska", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(3);
}
