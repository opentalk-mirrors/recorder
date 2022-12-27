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

    let mut mixer =
        Mixer::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, ()).unwrap();

    testing::add_overlay_name(&mut mixer, &format!("test_layout_{}", L::NAME));

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    let (_, ids) = testing::generate_streams(&mut mixer, 5, 5);

    mixer.play();

    mixer.generate_dot_file(&format!("test_layout_{}-0", L::NAME), testing::DOT_DETAILS);

    testing::wait();

    for i in 1..6 {
        title.set(&format!("Showing {i} Participant(s)"));

        mixer.pause();
        mixer.set_visibles(&ids[0..i]).unwrap();
        mixer.layout::<L>().unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_layout_{}-{i}", L::NAME),
            testing::DOT_DETAILS,
        );
        testing::wait();
    }

    testing::wait();
}

#[test]
fn test_remove() {
    // initialize for testing
    testing::init();

    let mut mixer =
        Mixer::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, ()).unwrap();

    testing::add_overlay_name(&mut mixer, "test_remove");

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    for i in 0..2 {
        testing::generate_streams(&mut mixer, 8, 5);

        mixer.generate_dot_file(&format!("test_remove_{i}-0"), testing::DOT_DETAILS);

        mixer.play();
        mixer.generate_dot_file(&format!("test_remove_{i}-1"), testing::DOT_DETAILS);

        testing::wait();

        title.set("remove 0 (left 1-7)");
        mixer.pause();
        mixer.remove_stream(0).unwrap();
        mixer.layout::<Grid>().unwrap();

        mixer.play();
        mixer.generate_dot_file(&format!("test_remove_{i}-2"), testing::DOT_DETAILS);

        testing::wait();

        title.set("remove 1-2 (left 3-7)");
        mixer.pause();
        mixer.remove_stream(1).unwrap();
        mixer.remove_stream(2).unwrap();
        mixer.layout::<Grid>().unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove_{i}-3"), testing::DOT_DETAILS);

        testing::wait();

        title.set("remove 3-6 (left 7)");
        mixer.pause();
        mixer.remove_stream(3).unwrap();
        mixer.remove_stream(4).unwrap();
        mixer.remove_stream(5).unwrap();
        mixer.remove_stream(6).unwrap();
        mixer.layout::<Grid>().unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove_{i}-4"), testing::DOT_DETAILS);

        testing::wait();

        title.set("remove 7 (none left)");
        mixer.pause();
        mixer.remove_stream(7).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove_{i}-5"), testing::DOT_DETAILS);

        // check if we cannot remove any of which we removed before
        assert!(mixer.remove_stream(0).is_err());
        assert!(mixer.remove_stream(1).is_err());
        assert!(mixer.remove_stream(2).is_err());
        assert!(mixer.remove_stream(3).is_err());
        assert!(mixer.remove_stream(4).is_err());
        assert!(mixer.remove_stream(5).is_err());
        assert!(mixer.remove_stream(6).is_err());
        assert!(mixer.remove_stream(7).is_err());

        testing::wait();

        mixer.pause();
    }
}
