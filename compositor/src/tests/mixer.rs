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
        Box::<testing::TestSinkBuilder>::default(),
        None,
    )
    .unwrap();

    testing::wait_millis(100);

    let (_, ids) = testing::generate_streams(&mut talk, 5, 5);

    talk.dot(&format!("test_layout_{}-0", L::NAME), testing::DOT_PARAMS);

    testing::wait();

    for i in 1..ids.len() + 1 {
        talk.set_title(&format!("Showing {i} Participant(s)"));

        talk.dot(&format!("test_layout_{}-{i}", L::NAME), testing::DOT_PARAMS);
        testing::wait();
    }

    testing::wait();
}

#[test]
fn test_remove() {
    // initialize for testing
    testing::init();

    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Box::<testing::TestSinkBuilder>::default(),
        None,
    )
    .unwrap();

    talk.set_title("test_remove");

    for i in 0..2 {
        testing::generate_streams(&mut talk, 8, 5);

        talk.dot(&format!("test_remove_{i}-0"), testing::DOT_PARAMS);

        talk.dot(&format!("test_remove_{i}-1"), testing::DOT_PARAMS);

        testing::wait();

        talk.set_title("remove 0 (left 1-7)");
        talk.remove_stream(StreamId::camera(0)).unwrap();
        talk.layout::<Grid>().unwrap();

        talk.dot(&format!("test_remove_{i}-2"), testing::DOT_PARAMS);

        testing::wait();

        talk.set_title("remove 1-2 (left 3-7)");
        talk.remove_stream(StreamId::camera(1)).unwrap();
        talk.remove_stream(StreamId::camera(2)).unwrap();
        talk.layout::<Grid>().unwrap();
        talk.dot(&format!("test_remove_{i}-3"), testing::DOT_PARAMS);

        testing::wait();

        talk.set_title("remove 3-6 (left 7)");
        talk.remove_stream(StreamId::camera(3)).unwrap();
        talk.remove_stream(StreamId::camera(4)).unwrap();
        talk.remove_stream(StreamId::camera(5)).unwrap();
        talk.remove_stream(StreamId::camera(6)).unwrap();
        talk.layout::<Grid>().unwrap();
        talk.dot(&format!("test_remove_{i}-4"), testing::DOT_PARAMS);

        testing::wait();

        talk.set_title("remove 7 (none left)");
        talk.remove_stream(StreamId::camera(7)).unwrap();
        talk.dot(&format!("test_remove_{i}-5"), testing::DOT_PARAMS);

        testing::wait();
    }
}
