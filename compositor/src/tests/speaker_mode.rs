// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::core::ParticipantId;
use types::signaling::media::{MediaSessionState, MediaSessionType};

use crate::{
    testing, MediaDescriptor, Mixer, Pattern, Size, Speaker, TestSink, TestSource,
    TestSourceParameters,
};

#[test]
#[ignore = "failing in ci"]
fn test_speaker_mode_without_prio() {
    testing::init();

    const MAX_VISIBLES: usize = 5;
    const NUM_PARTICIPANTS: usize = 10;

    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        Speaker::default(),
        MAX_VISIBLES,
        true,
    )
    .unwrap();
    mixer
        .link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();

    mixer.set_title("test_speaker_mode_without_prio");

    let (streams, _) =
        testing::generate_streams(&mut mixer, 0, NUM_PARTICIPANTS as u32, MAX_VISIBLES, true);

    for stream in &streams[0..NUM_PARTICIPANTS] {
        mixer.set_title(&format!("Speaker: {}", stream.1));

        mixer.set_speaker(stream.0).unwrap();

        mixer.dot(
            &format!("test_speaker_mode_without_prio-{id:?}", id = stream.0),
            testing::DOT_PARAMS,
        );

        testing::wait();
    }
}

#[test]
#[ignore = "failing in ci"]
fn test_speaker_mode_with_prio() {
    testing::init();

    const MAX_VISIBLES: usize = 5;
    const NUM_PARTICIPANTS: usize = 10;

    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        Speaker::default(),
        MAX_VISIBLES,
        true,
    )
    .unwrap();
    mixer
        .link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();

    mixer.set_title("test_speaker_mode_with_prio");

    let (streams, _) =
        testing::generate_streams(&mut mixer, 0, NUM_PARTICIPANTS as u32, MAX_VISIBLES, true);

    mixer
        .add_stream(
            MediaDescriptor {
                participant_id: streams[0].0,
                media_type: MediaSessionType::Screen,
            },
            format!("{}'s screen", streams[0].1),
            TestSourceParameters {
                resolution: Size::SD,
                name: Some(format!("{}'s screen", streams[0].1)),
                pattern: Pattern::Smpte75,
                has_video: true,
            },
            MediaSessionState::audio_and_video(),
        )
        .unwrap();

    mixer.dot("test_speaker_mode_with_prio-0", testing::DOT_PARAMS);

    testing::wait();

    for stream in &streams[0..NUM_PARTICIPANTS] {
        mixer.set_title(&format!("Speaker: {}", stream.1));

        mixer.set_speaker(stream.0).unwrap();

        mixer.dot(
            &format!("test_speaker_mode_with_prio-{id:?}", id = stream.0),
            testing::DOT_PARAMS,
        );

        testing::wait();
    }
}
