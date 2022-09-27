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
    let mixer = mixer::Mixer::new(&pipeline, &resolution, args.visibles, layout, args.display);
    let output = mixer::DisplaySink::create(&pipeline, &resolution);
    mixer.link_display_sink(&output);

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

    let mut participants = Vec::<mixer::Participant>::new();
    for n in 0..80 {
        participants.push(mixer::Participant::create(
            &pipeline,
            &format!("{n}"),
            "smpte",
            &resolution,
        ));
    }

    pipeline.set_state(gst::State::Playing);

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer::generate_dot_file(&pipeline, "pipeline-null");
    }

    for m in 0..100 {
        for n in 0..participants.len() {
            participants[n].link(&mixer).unwrap();
            std::thread::sleep_ms(1000);
        }
        std::thread::sleep_ms(500);
        for n in 0..participants.len() {
            participants[n].unlink(&mixer).unwrap();
            std::thread::sleep_ms(1000);
        }
    }
    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer::generate_dot_file(&pipeline, "pipeline");
    }

    // clone a mixer instance for the thread
    let pipeline = pipeline.clone();

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
