// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, MediaDescriptor, Mixer, Speaker, TestSink, TestSource};
use types::core::ParticipantId;
use types::signaling::media::MediaSessionType;

#[test]
fn test_overlay() {
    // initialize for testing
    testing::init();

    // get output resolution from arguments
    let mut mixer = Mixer::<TestSource>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();
    mixer
        .link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();

    mixer.set_title("test_overlay").unwrap();

    mixer.dot("test_overlay-0", testing::DOT_PARAMS);

    testing::wait();

    mixer.dot("test_overlay-1", testing::DOT_PARAMS);

    testing::wait();

    // add participants
    let (_, ids) = testing::generate_streams(&mut mixer, 0, 3, 3, true);
    ids.iter().for_each(|id| {
        mixer
            .show_stream(&MediaDescriptor {
                participant_id: *id,
                media_type: MediaSessionType::Video,
            })
            .unwrap();
    });
    mixer.dot("test_overlay-3", testing::DOT_PARAMS);

    testing::wait();

    for id in ids {
        // add text overlay to source
        mixer
            .set_stream_title(
                &MediaDescriptor {
                    participant_id: id,
                    media_type: MediaSessionType::Video,
                },
                "new text",
            )
            .unwrap();
        mixer.dot("test_overlay-4", testing::DOT_PARAMS);
        testing::wait();
    }

    testing::wait_secs(10);
}
