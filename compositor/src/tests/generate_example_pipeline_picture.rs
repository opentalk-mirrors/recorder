use crate::{testing::RESOLUTION, *};

const IMAGE_OUTPUT_PATH: &str = "./images";

/// generate an example of a usual pipeline
#[test]
fn generate_example_pipeline_picture() {
    // initialize logging
    let _ = env_logger::try_init();

    // initialize GStreamer
    gst::init().unwrap();

    let dp = &debug::Params {
        index: false,
        ..debug::Params::states()
    };

    // setup mixer
    let mut talk =
        Talk::<TestSource, testing::TestSink, u32>::new(RESOLUTION, Default::default(), None)
            .unwrap();
    // generate pipeline DOT graph of the empty pipeline
    talk.dot("0_init", dp);

    // add three streams
    for i in 0..3 {
        talk.add_stream(
            StreamId::camera(i),
            &format!("P{i}]"),
            Default::default(),
            StreamStatus::default(),
        )
        .unwrap();
    }

    talk.dot("1_add_streams", dp);

    talk.dot("2_overlay", dp);
    talk.set_title("text");

    // set two streams to be visible
    talk.layout::<Grid>().unwrap();
    talk.dot("3_set_visibles", dp);

    talk.dot("example_pipeline", dp);

    info!("converting dot files into png...");
    convert("0_init");
    convert("1_add_streams");
    convert("2_overlay");
    convert("3_set_visibles");
    convert("example_pipeline");
}

/// check whether the generated PNG equals the old one before overwriting it
fn convert(name: &str) {
    let dot_path = "pipelines";
    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", dot_path);
    let dot = &format!("{dot_path}/{name}.dot");
    let intermediate = &format!("{IMAGE_OUTPUT_PATH}/{name}.new.png");
    let png = &format!("{IMAGE_OUTPUT_PATH}/{name}.png");
    std::fs::create_dir_all(IMAGE_OUTPUT_PATH).expect("can not create dir from IMAGE_OUTPUT_PATH");
    // check
    match std::process::Command::new("dot").arg("-h").output() {
        Ok(_) => {
            let dot_out = std::process::Command::new("dot")
                .args(["-Tpng", "-o", intermediate, dot])
                .output()
                .expect("command 'dot' failed to generate a PNG");
            if !dot_out.status.success() {
                panic!("dot generation did not work. file: {}", dot);
            }
            let id_intermediate = std::process::Command::new("identify")
                .args(["-quiet", "-format", "%#", intermediate])
                .output()
                .expect("command 'identify' failed")
                .stdout;
            let id_png = std::process::Command::new("identify")
                .args(["-quiet", "-format", "%#", png])
                .output()
                .expect("command 'identify' failed")
                .stdout;

            if id_intermediate != id_png {
                info!("updating file '{png}'");
                std::fs::copy(intermediate, png).unwrap_or_else(|_| {
                    panic!("command 'copy' failed for {intermediate} -> {png}")
                });
            }
            std::fs::remove_file(intermediate).unwrap();
        }
        Err(_) => {
            warn!("install imagemagick to optimize update");
            info!("updating file '{png}'");
            std::fs::copy(intermediate, png).expect("command 'copy' failed");
        }
    }
}
