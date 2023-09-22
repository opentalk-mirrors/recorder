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

    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, testing::TestSink::default(), None)
            .unwrap();

    testing::wait_millis(100);

    let (_, ids) = testing::generate_streams(&mut talk, 5, 5);
    talk.layout::<L>().unwrap();

    talk.dot(&format!("test_layout_{}-0", L::NAME), testing::DOT_PARAMS);

    testing::wait();

    (0..ids.len()).for_each(|i| {
        talk.set_title(&format!("Showing {i} Participant(s)", i = i + 1));
        talk.try_show(&StreamId::camera(ids[i]));
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

    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, testing::TestSink::default(), None)
            .unwrap();

    talk.set_title("test_remove");

    for _ in 0..50 {
        testing::generate_streams(&mut talk, 8, 5);
        talk.set_speaker(Some(1), &Default::default()).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove-0", testing::DOT_PARAMS);

        testing::wait_short();

        talk.set_title("remove 0 (left 1-7)");
        talk.remove_stream(StreamId::camera(0)).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove-1", testing::DOT_PARAMS);

        testing::wait_short();

        talk.set_title("remove 1-2 (left 3-7)");
        talk.remove_stream(StreamId::camera(1)).unwrap();
        talk.remove_stream(StreamId::camera(2)).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_2", testing::DOT_PARAMS);

        testing::wait_short();

        talk.set_title("remove 3-6 (left 7)");
        talk.remove_stream(StreamId::camera(3)).unwrap();
        talk.remove_stream(StreamId::camera(4)).unwrap();
        talk.remove_stream(StreamId::camera(5)).unwrap();
        talk.remove_stream(StreamId::camera(6)).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_3", testing::DOT_PARAMS);

        testing::wait_short();

        talk.set_title("remove 7 (none left)");
        talk.remove_stream(StreamId::camera(7)).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot("test_remove_4", testing::DOT_PARAMS);

        testing::wait_short();
    }
}
