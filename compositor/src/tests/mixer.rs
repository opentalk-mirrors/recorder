// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::signaling::media::MediaSessionType;

use crate::{testing, Grid, Layout, MediaDescriptor, Mixer, Speaker, TestSink, TestSource};

#[test]
fn test_layout_speaker() {
    test_layout(Speaker::default(), "speaker");
}

#[test]
fn test_layout_grid() {
    test_layout(Grid::default(), "grid");
}

fn test_layout(layout: impl Layout, name: &str) {
    // initialize for testing
    testing::init();

    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        layout,
        testing::MAX_STREAMS,
        true,
        &Default::default(),
    )
    .unwrap();

    let test_sink = TestSink::create("Testing Sink", true).unwrap();

    mixer.link_sink("test_sink", test_sink).unwrap();

    testing::wait_millis(100);

    let (_, ids) = testing::generate_streams(&mut mixer, 0, 5, 5, true);

    mixer.dot(&format!("test_layout_{}-0", name), testing::DOT_PARAMS);

    testing::wait();

    ids.iter().enumerate().for_each(|(index, id)| {
        mixer.set_title(&format!(
            "Showing {amount} Participant(s)",
            amount = index + 1
        ));
        mixer
            .show_stream(MediaDescriptor {
                participant_id: *id,
                media_type: MediaSessionType::Video,
            })
            .unwrap();
        mixer.dot(&format!("test_layout_{name}-{index}"), testing::DOT_PARAMS);
        testing::wait();
    });

    testing::wait_secs(10);
}

fn test_remove(use_video: bool) {
    // initialize for testing
    testing::init();

    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        use_video,
        &Default::default(),
    )
    .unwrap();

    mixer
        .link_sink(
            "test_sink",
            TestSink::create("Recording", use_video).unwrap(),
        )
        .unwrap();

    mixer.set_title("test_remove");

    for i in 0..50 {
        let (_, ids) = testing::generate_streams(&mut mixer, i * 8, 8, 5, use_video);
        for id in &ids {
            mixer
                .show_stream(MediaDescriptor {
                    participant_id: *id,
                    media_type: MediaSessionType::Video,
                })
                .unwrap();
        }
        mixer.set_speaker(ids[0]).unwrap();

        mixer.dot("test_remove-0", testing::DOT_PARAMS);

        testing::wait();

        mixer.set_title(&format!(
            "remove {id0:?} (left {id1:?}-{id7:?})",
            id0 = ids[0],
            id1 = ids[1],
            id7 = ids[7]
        ));
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[0],
                media_type: MediaSessionType::Video,
            })
            .unwrap();

        mixer.dot("test_remove-1", testing::DOT_PARAMS);

        testing::wait();

        mixer.set_title(&format!(
            "remove {id1:?}-{id2:?} (left {id3:?}-{id7:?})",
            id1 = ids[1],
            id2 = ids[2],
            id3 = ids[3],
            id7 = ids[7],
        ));
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[1],
                media_type: MediaSessionType::Video,
            })
            .unwrap();
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[2],
                media_type: MediaSessionType::Video,
            })
            .unwrap();

        mixer.dot("test_remove_2", testing::DOT_PARAMS);

        testing::wait();

        mixer.set_title(&format!(
            "remove {id3:?}-{id6:?} (left {id7:?})",
            id3 = ids[3],
            id6 = ids[6],
            id7 = ids[7],
        ));
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[3],
                media_type: MediaSessionType::Video,
            })
            .unwrap();
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[4],
                media_type: MediaSessionType::Video,
            })
            .unwrap();
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[5],
                media_type: MediaSessionType::Video,
            })
            .unwrap();
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[6],
                media_type: MediaSessionType::Video,
            })
            .unwrap();

        mixer.dot("test_remove_3", testing::DOT_PARAMS);

        testing::wait();

        mixer.set_title(&format!("remove {id7:?} (none left)", id7 = ids[7]));
        mixer
            .remove_stream(MediaDescriptor {
                participant_id: ids[7],
                media_type: MediaSessionType::Video,
            })
            .unwrap();

        mixer.dot("test_remove_4", testing::DOT_PARAMS);

        testing::wait();
    }

    testing::wait_secs(10);
}

#[test]
fn test_remove_video() {
    test_remove(true);
}

#[test]
fn test_remove_audio() {
    test_remove(false);
}
