use crate::*;

#[test]
fn test_overlay() {
    // initialize for testing
    testing::init();

    // get output resolution from arguments
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Box::new(testing::TestSinkBuilder::new()),
        None,
    )
    .unwrap();

    testing::add_overlay_name(&mut talk, "test_overlay");

    talk.dot("test_overlay-0", testing::DOT_PARAMS);

    testing::wait();

    // add clock overlay
    talk.insert_overlay_clock("Clock Overlay: %x %X %Z", TextFormat::default())
        .unwrap();
    talk.dot("test_overlay-1", testing::DOT_PARAMS);

    testing::wait();

    // add text overlay
    talk.insert_overlay_text(
        "Text Overlay",
        TextFormat {
            align: Align {
                vertical: VAlign::Top,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    talk.dot("test_overlay-2", testing::DOT_PARAMS);

    testing::wait();

    // add participants
    let (_, ids) = testing::generate_streams(&mut talk, 3, 3);
    talk.dot("test_overlay-3", testing::DOT_PARAMS);

    testing::wait();

    for id in ids {
        // add text overlay to source
        talk.insert_source_overlay_text(&id.into(), "Source Text Overlay", TextFormat::default())
            .unwrap();
        talk.dot("test_overlay-4", testing::DOT_PARAMS);
        testing::wait();
    }
}
