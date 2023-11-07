// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::*;

#[test]
fn test_layout_speaker() {
    test_layout::<Speaker>();
}

#[test]
fn test_layout_grid() {
    test_layout::<Grid>();
}

fn test_layout<L>()
where
    L: Layout,
{
    // initialize for testing
    testing::init();

    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        TestSink::default(),
        testing::MAX_STREAMS,
    )
    .unwrap();

    testing::wait_millis(100);

    let (_, ids) = testing::generate_streams(&mut talk, 0, 5, 5);
    talk.layout::<L>().unwrap();

    talk.dot(&format!("test_layout_{}-0", L::NAME), testing::DOT_PARAMS);

    testing::wait();

    (0..ids.len()).for_each(|i| {
        talk.set_title(&format!("Showing {i} Participant(s)", i = i + 1));
        talk.show_stream(&StreamId::camera(ids[i]));
        talk.layout::<L>().unwrap();
        talk.dot(&format!("test_layout_{}-{i}", L::NAME), testing::DOT_PARAMS);
        testing::wait();
    });

    testing::wait();
}

#[test]
fn test_remove() {
    // initialize for testing
    testing::init();

    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        TestSink::default(),
        testing::MAX_STREAMS,
    )
    .unwrap();

    talk.set_title("test_remove");

    for i in 0..50 {
        let (_, ids) = testing::generate_streams(&mut talk, i * 8, 8, 5);
        for id in &ids {
            talk.show_stream(&StreamId::camera(*id));
        }
        talk.set_speaker(ids[0]);
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove-0", testing::DOT_PARAMS);

        testing::wait();

        talk.set_title(&format!(
            "remove {id0} (left {id1}-{id7})",
            id0 = ids[0],
            id1 = ids[1],
            id7 = ids[7]
        ));
        talk.remove_stream(StreamId::camera(ids[0])).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove-1", testing::DOT_PARAMS);

        testing::wait();

        talk.set_title(&format!(
            "remove {id1}-{id2} (left {id3}-{id7})",
            id1 = ids[1],
            id2 = ids[2],
            id3 = ids[3],
            id7 = ids[7],
        ));
        talk.remove_stream(StreamId::camera(ids[1])).unwrap();
        talk.remove_stream(StreamId::camera(ids[2])).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_2", testing::DOT_PARAMS);

        testing::wait();

        talk.set_title(&format!(
            "remove {id3}-{id6} (left {id7})",
            id3 = ids[3],
            id6 = ids[6],
            id7 = ids[7],
        ));
        talk.remove_stream(StreamId::camera(ids[3])).unwrap();
        talk.remove_stream(StreamId::camera(ids[4])).unwrap();
        talk.remove_stream(StreamId::camera(ids[5])).unwrap();
        talk.remove_stream(StreamId::camera(ids[6])).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_3", testing::DOT_PARAMS);

        testing::wait();

        talk.set_title(&format!("remove {id7} (none left)", id7 = ids[7]));
        talk.remove_stream(StreamId::camera(ids[7])).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_4", testing::DOT_PARAMS);

        testing::wait();
    }
}
