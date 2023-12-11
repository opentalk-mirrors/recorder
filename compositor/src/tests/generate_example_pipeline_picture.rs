// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{
    debug, testing, Blinder, Speaker, Talk, TestBlinder, TestBlinderParams, TestSink, TestSource,
    TestSourceParameters,
};

const IMAGE_OUTPUT_PATH: &str = "./images";
const DOT_PATH: &str = "pipelines";

/// generate an example of a usual pipeline
#[test]
#[ignore = "this is generating the pipeline pictues and should only be run manually"]
fn generate_example_pipeline_picture() {
    // initialize logging
    let _ = env_logger::try_init();

    // initialize GStreamer
    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_PATH);
    gst::init().unwrap();

    let dp = &debug::Params {
        index: false,
        ..debug::Params::states()
    };

    let blinder = Box::new(
        TestBlinder::create(&TestBlinderParams {
            name: "Testing Blinder",
            sink: Box::new(TestSink::create("Streaming", true).unwrap()),
            resolution: testing::RESOLUTION,
            alt_source_params: TestSourceParameters::default(),
        })
        .unwrap(),
    );

    // setup mixer
    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, Speaker::default(), 100, true).unwrap();

    talk.link_sink("test_sink", TestSink::create("Recording", true).unwrap())
        .unwrap();

    testing::generate_streams(&mut talk, 0, 3, 3, true);
    talk.set_speaker(0).unwrap();
    blinder.blind(false);

    talk.set_title("not blinded").unwrap();
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(true);

    talk.set_title("blinded").unwrap();
    talk.dot("test_blinder-blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(false);

    talk.set_title("not blinded").unwrap();
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();
    talk.set_title("shutdown").unwrap();

    talk.dot("example_pipeline", dp);

    info!("converting dot files into png...");
    convert("example_pipeline");
}

/// check whether the generated PNG equals the old one before overwriting it
fn convert(name: &str) {
    let dot = &format!("{DOT_PATH}/{name}.dot");
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
