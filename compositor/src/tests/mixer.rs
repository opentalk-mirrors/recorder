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
        Box::new(testing::TestSinkBuilder::new()),
        None,
    )
    .unwrap();

    testing::add_overlay_name(&mut talk, &format!("test_layout_{}", L::NAME));

    let title = talk.insert_overlay_text("", TextFormat::default()).unwrap();

    testing::wait_millis(100);

    let (_, ids) = testing::generate_streams(&mut talk, 5, 5);

    talk.dot(&format!("test_layout_{}-0", L::NAME), testing::DOT_PARAMS);

    testing::wait();

    for i in 1..ids.len() + 1 {
        title.set(&format!("Showing {i} Participant(s)"));

        talk.dot(&format!("test_layout_{}-{i}", L::NAME), testing::DOT_PARAMS);
        testing::wait();
    }

    testing::wait();
}

#[test]
fn test_remove() {
    // initialize for testing
    testing::init();

    let mut mixer = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Box::new(testing::TestSinkBuilder::new()),
        None,
    )
    .unwrap();

    testing::add_overlay_name(&mut mixer, "test_remove");

    let title = mixer
        .insert_overlay_text("", TextFormat::default())
        .unwrap();

    for i in 0..2 {
        testing::generate_streams(&mut mixer, 8, 5);

        mixer.dot(&format!("test_remove_{i}-0"), testing::DOT_PARAMS);

        mixer.dot(&format!("test_remove_{i}-1"), testing::DOT_PARAMS);

        testing::wait();

        title.set("remove 0 (left 1-7)");
        mixer.remove_stream(0.into()).unwrap();
        mixer.layout::<Grid>().unwrap();

        mixer.dot(&format!("test_remove_{i}-2"), testing::DOT_PARAMS);

        testing::wait();

        title.set("remove 1-2 (left 3-7)");
        mixer.remove_stream(1.into()).unwrap();
        mixer.remove_stream(2.into()).unwrap();
        mixer.layout::<Grid>().unwrap();
        mixer.dot(&format!("test_remove_{i}-3"), testing::DOT_PARAMS);

        testing::wait();

        title.set("remove 3-6 (left 7)");
        mixer.remove_stream(3.into()).unwrap();
        mixer.remove_stream(4.into()).unwrap();
        mixer.remove_stream(5.into()).unwrap();
        mixer.remove_stream(6.into()).unwrap();
        mixer.layout::<Grid>().unwrap();
        mixer.dot(&format!("test_remove_{i}-4"), testing::DOT_PARAMS);

        testing::wait();

        title.set("remove 7 (none left)");
        mixer.remove_stream(7.into()).unwrap();
        mixer.dot(&format!("test_remove_{i}-5"), testing::DOT_PARAMS);

        testing::wait();
    }
}
