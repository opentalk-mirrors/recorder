mod mixer;

extern crate clap;
#[macro_use]
extern crate log;

use clap::Parser;
use gst::traits::{ElementExt, GstBinExt, GstObjectExt};
use gstreamer as gst;
use mixer::*;

/// program arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Arguments {
    /// number of visible viewers (additionally to the speaker)
    #[clap(short, long, value_parser, default_value = "5")]
    participants: usize,
    /// width and height (e.g. `1920x1080`) of the composite output
    #[clap(long, value_parser, default_value = "640x480")]
    resolution: String,
    /// just use video display instead of recording
    #[clap(short, long)]
    display: bool,
    /// generate dot file of pipeline
    #[clap(short = 'D', long)]
    dot: bool,
    /// use test sources
    #[clap(short, long)]
    test: bool,
}

fn main() {
    // initialize logger
    env_logger::init();

    // parse command line arguments
    let args = Arguments::parse();

    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = {
        let x = args
            .resolution
            .split('x')
            .map(|x| x.parse().unwrap())
            .collect::<Vec<usize>>();
        Size {
            width: x[0],
            height: x[1],
        }
    };

    let layout = SpeakerLayout::new(&resolution);
    // create a mixer for audio and video
    let mut mixer = Mixer::new(&resolution, args.display);

    let names = [
        "Peer",
        "Markus",
        "Konstantin",
        "Pat",
        "Stefan",
        "Michael",
        "Dennis",
        "A",
        "B",
        "C",
        "D",
        "E",
    ];

    for (n, name) in names[0..args.participants].iter().enumerate() {
        // std::thread::sleep_ms(2000);
        mixer.add_test_source(&layout, name, &resolution);
        mixer.set_viewable(&names[0..n]);
    }
    // mixer
    //     .pipeline
    //     .by_name("bin0")
    //     .unwrap()
    //     .set_state(gst::State::Playing)
    //     .unwrap();

    std::thread::sleep_ms(2000);

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            info!("writing DOT file `{}/pipeline.dot`...", path);
            mixer.generate_dot_file("pipeline");
        } else {
            warn!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
        }
    }

    // clone a mixer instance for the thread
    let pipeline = mixer.pipeline.clone();

    // start thread which continuously switches speaking text
    std::thread::spawn(move || {
        // initially set title
        mixer.set_title("Some very important meeting");

        let mut i: usize = 0;
        loop {
            // continuously set who's speaking
            mixer.set_speaking(&format!("{}", names[i % names.len()]));
            // and switch who's speaking
            i += 1;
            if i >= names.len() {
                i = 0;
            }
            // take time
            std::thread::sleep(std::time::Duration::from_millis(2000));
        }
    });

    // run the mixer
    run(&pipeline);
}

/// wait until mixer generates error or ends
pub fn run(pipeline: &gst::Pipeline) {
    // wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                eprintln!("Debugging information: {:?}", err.debug());
                break;
            }
            MessageView::Eos(..) => break,
            _ => (),
        }
    }

    // stop pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
