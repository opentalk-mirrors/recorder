use crate::{testing::RESOLUTION, *};

const DOT_OUTPUT_PATH: &str = "./pipelines";
const IMAGE_OUTPUT_PATH: &str = "./images";

/// generate an example of a usual pipeline
#[test]
fn generate_example_pipeline_picture() {
    // initialize logging
    let _ = env_logger::try_init();

    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_OUTPUT_PATH);

    // initialize GStreamer
    gst::init().unwrap();

    let dp = &debug::Params {
        index: false,
        ..debug::Params::states()
    };

    // setup mixer
    let mut talk = Talk::<TestSource, FakeSink, u32>::new(RESOLUTION, (), None).unwrap();
    // generate pipeline DOT graph of the empty pipeline
    talk.dot("0_init", dp);

    // prepare test source parameters
    let params = TestSourceParameters::default();

    // add three streams
    for i in 0..3 {
        talk.add_stream(
            i.into(),
            format!("P{i}]"),
            params.clone(),
            StreamStatus::default(),
        )
        .unwrap();
    }

    talk.dot("1_add_streams", dp);

    talk.dot("2_overlay", dp);
    talk.insert_overlay_text("text", Default::default())
        .unwrap();

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
    let dot = &format!("{DOT_OUTPUT_PATH}/{name}.dot");
    let intermediate = &format!("{IMAGE_OUTPUT_PATH}/{name}.new.png");
    let png = &format!("{IMAGE_OUTPUT_PATH}/{name}.png");
    // check
    match std::process::Command::new("dot").arg("-h").output() {
        Ok(_) => {
            std::process::Command::new("dot")
                .args(["-Tpng", "-o", intermediate, dot])
                .output()
                .unwrap();
            let id_intermediate = std::process::Command::new("identify")
                .args(["-quiet", "-format", "%#", intermediate])
                .output()
                .unwrap()
                .stdout;
            let id_png = std::process::Command::new("identify")
                .args(["-quiet", "-format", "%#", png])
                .output()
                .unwrap()
                .stdout;

            if id_intermediate != id_png {
                info!("updating file '{png}'");
                std::fs::copy(intermediate, png).unwrap();
            }
            std::fs::remove_file(intermediate).unwrap();
        }
        Err(_) => {
            warn!("install imagemagick to optimize update");
            info!("updating file '{png}'");
            std::fs::copy(intermediate, png).unwrap();
        }
    }
}
