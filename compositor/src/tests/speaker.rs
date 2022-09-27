#[test]
fn test_speaker_view() {
    env_logger::init();
    gst::init().unwrap();
    let pipeline = gst::Pipeline::new(None);
    let mut mixer = mixer::Mixer::new(
        &pipeline,
        &resolution,
        args.visibles,
        layout::Grid::new(&Size {
            width: 320,
            height: 240,
        }),
        args.display,
    );
    let output = mixer::DisplaySink::new(&pipeline, &resolution);
    mixer.link_display_sink(&output);
    pipeline.set_state(gst::State::Playing).unwrap();

    // start thread which continuously switches speaking text
    std::thread::spawn({
        let pipeline = pipeline.clone();
        move || {
            mixer.set_title("Some very important meeting");

            for n in 0..args.participants {
                let name = format!("{n}");
                mixer.participants.insert(
                    name.clone(),
                    mixer::Participant::create(&pipeline, &name, "smpte", &resolution),
                );
            }
            trace!("participants: {:?}", mixer.participants.keys());

            let mut i: usize = 0;
            let mut m: isize = 0;
            let mut step: isize = 1;
            loop {
                trace!("==================================================================");
                let mut names = Vec::new();
                for i in 0..m {
                    names.push(format!("{i}"));
                }
                trace!("-set_viewable-----------------------------------------------------");
                trace!("visibles: {:?}", names);
                pipeline.set_state(gst::State::Paused).unwrap();
                std::thread::sleep_ms(100);
                mixer.set_viewable(&names);
                // continuously set who's speaking
                if i > 0 {
                    mixer.set_speaking(&format!("{}", names[i % names.len()]));
                }
                trace!("-layout-----------------------------------------------------------");
                mixer.layout();

                pipeline.set_state(gst::State::Playing).unwrap();
                trace!("-ready-----------------------------------------------------------");
                std::thread::sleep_ms(100);

                // shall we create DOT file of mixer's pipeline?
                if args.dot {
                    // must set this to work
                    mixer::generate_dot_file(&pipeline, &format!("pipeline-{i}-{m}"));
                }
                // and switch who's speaking
                i += 1;
                if i >= names.len() {
                    i = 0;
                }

                if m == args.visibles as isize - 1 {
                    step = -1;
                }
                if m == 0 {
                    step = 1;
                }
                m = m + step;
            }
        }
    });

    /// wait until mixer generates error or ends
    mixer.run();
}
