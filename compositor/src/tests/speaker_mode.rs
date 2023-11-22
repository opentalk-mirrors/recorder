// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{
    testing, Pattern, Size, Speaker, StreamId, StreamStatus, Talk, TestSink, TestSource,
    TestSourceParameters,
};

#[test]
#[ignore = "failing in ci"]
fn test_speaker_mode_without_prio() {
    testing::init();

    const MAX_VISIBLES: usize = 5;
    const NUM_PARTICIPANTS: usize = 10;

    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, Speaker::default(), MAX_VISIBLES, true)
            .unwrap();
    talk.link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    talk.set_speaker(0).unwrap();

    talk.set_title("test_speaker_mode_without_prio").unwrap();

    let (streams, _) =
        testing::generate_streams(&mut talk, 0, NUM_PARTICIPANTS as u32, MAX_VISIBLES, true);

    for stream in &streams[0..NUM_PARTICIPANTS] {
        talk.set_title(&format!("Speaker: {}", stream.1)).unwrap();

        talk.set_speaker(stream.0).unwrap();

        talk.dot(
            &format!("test_speaker_mode_without_prio-{}", stream.0 + 1),
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

    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, Speaker::default(), MAX_VISIBLES, true)
            .unwrap();
    talk.link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    talk.set_speaker(0).unwrap();

    talk.set_title("test_speaker_mode_with_prio").unwrap();

    let (streams, _) =
        testing::generate_streams(&mut talk, 0, NUM_PARTICIPANTS as u32, MAX_VISIBLES, true);

    talk.add_stream(
        StreamId::screen(streams[0].0),
        &format!("{}'s screen", streams[0].1),
        TestSourceParameters {
            resolution: Size::SD,
            name: Some(format!("{}'s screen", streams[0].1)),
            pattern: Pattern::Smpte75,
            has_video: true,
        },
        StreamStatus::default(),
    )
    .unwrap();

    talk.dot("test_speaker_mode_with_prio-0", testing::DOT_PARAMS);

    testing::wait();

    for stream in &streams[0..NUM_PARTICIPANTS] {
        talk.set_title(&format!("Speaker: {}", stream.1)).unwrap();

        talk.set_speaker(stream.0).unwrap();

        talk.dot(
            &format!("test_speaker_mode_with_prio-{}", stream.0 + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();
    }
}
