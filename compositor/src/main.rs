mod error;
mod layout;
mod mixer;

extern crate clap;
#[macro_use]
extern crate log;

use clap::Parser;
use gst::traits::{ElementExt, GstObjectExt};
use gstreamer as gst;

/// program arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Arguments {
    /// number of visible viewers (additionally to the speaker)
    #[clap(short, long, value_parser, default_value = "5")]
    participants: usize,
    /// maximum number of visible participants
    #[clap(short, long, value_parser, default_value = "5")]
    visibles: usize,
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
        layout::Size {
            width: x[0],
            height: x[1],
        }
    };

    let layout = layout::Grid::new(&resolution);
    let pipeline = gst::Pipeline::new(None);
    // create a mixer for audio and video
    let mut mixer = mixer::Mixer::new(&pipeline, &resolution, args.visibles, layout, args.display);
    let output = mixer::DisplaySink::create(&pipeline, &resolution);
    mixer.link_display_sink(&output);

    pipeline.set_state(gst::State::Playing).unwrap();

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer::generate_dot_file(&pipeline, "pipeline");
    }

    // clone a mixer instance for the thread
    let pipeline_ = pipeline.clone();

    // start thread which continuously switches speaking text
    std::thread::spawn(move || {
        // initially set title
        mixer.set_title("Some very important meeting");

        for n in 0..args.participants {
            let name = format!("{n}");
            mixer.participants.insert(
                name.clone(),
                mixer::Participant::create(&pipeline_, &name, "smpte", &resolution),
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
            mixer.set_viewable(&names);
            // continuously set who's speaking
            if i > 0 {
                mixer.set_speaking(&format!("{}", names[i % names.len()]));
            }
            // and switch who's speaking
            i += 1;
            if i >= names.len() {
                i = 0;
            }
            std::thread::sleep_ms(100);
            mixer.layout(&pipeline_);

            // shall we create DOT file of mixer's pipeline?
            if args.dot {
                // must set this to work
                mixer::generate_dot_file(&pipeline_, &format!("pipeline-{i}-{m}"));
            }

            if m == args.visibles as isize - 1 {
                step = -1;
            }
            if m == 0 {
                step = 1;
            }
            m = m + step;
            std::thread::sleep_ms(1000);

            // take time
            std::thread::sleep(std::time::Duration::from_millis(1000));
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
