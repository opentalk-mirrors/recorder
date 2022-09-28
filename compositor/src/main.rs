mod error;
mod layout;
mod mixer;
//mod tests;

extern crate clap;
#[macro_use]
extern crate log;

use clap::Parser;
use gstreamer as gst;
use layout::*;
use mixer::*;

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

    // create a mixer for audio and video
    let mixer = Mixer::<Grid, TestSource>::new::<DisplaySink>(&resolution, args.participants);

    mixer.play();

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer.generate_dot_file("pipeline");
    }

    // start thread which continuously switches speaking text
    std::thread::spawn({
        let mut mixer = mixer.clone();
        move || {
            // clone a mixer instance for the thread
            // initially set title
            mixer.set_title("Some very important meeting");

            // add participant names
            let names: Vec<String> = (0..args.participants)
                .enumerate()
                .map(|(n, _)| n.to_string().clone())
                .collect();
            mixer.add_participants(&names);

            let mut i: usize = 0;
            let mut m: isize = 0;
            let mut step: isize = 1;
            loop {
                // pause before changing the scene
                mixer.pause();

                // select participant names for visibility
                let names: Vec<String> = (0..m)
                    .enumerate()
                    .map(|(n, _)| n.to_string().clone())
                    .collect();
                mixer.set_viewable(&names);

                // continuously set who's speaking
                if !names.is_empty() {
                    mixer.set_speaking(&format!("{}", names[i % names.len()]));
                    // enumerate names
                    i += 1;
                }
                mixer.layout();

                // play after changing the scene
                mixer.play();

                // shall we create DOT file of mixer's pipeline?
                if args.dot {
                    // must set this to work
                    mixer.generate_dot_file(&format!("pipeline-{i}-{m}"));
                }

                // enumerate number of visibles up and down in a loop
                if m == args.visibles as isize - 1 {
                    step = -1;
                } else if m == 0 {
                    step = 1;
                }
                m += step;
            }
        }
    });

    // run the mixer
    mixer.run();
}
