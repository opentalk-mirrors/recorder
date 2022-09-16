mod mixer;

extern crate clap;
#[macro_use]
extern crate log;

use clap::Parser;
use gstreamer as gst;
use mixer::*;

/// program arguments
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Arguments {
    /// number of visible viewers (additionally to the speaker)
    #[clap(long, value_parser, default_value = "5")]
    viewers: usize,
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

    // create a mixer for audio and video
    let mixer = Mixer::new(args.viewers, resolution, args.display, args.test);

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
    let thread_mixer = mixer.clone();

    // start thread which continuously switches speaking text
    std::thread::spawn(move || {
        // initially set title
        thread_mixer.set_title("Some very important meeting");

        let mut i: usize = 0;
        let names = ["Peer", "Markus", "Konstantin", "Pat", "Stefan", "Michael"];
        loop {
            // continuously set who's speaking
            thread_mixer.set_speaking(&format!("{}", names[i]));
            // and switch who's speaking
            i += 1;
            if i > names.len() {
                i = 0;
            }
            // take time
            std::thread::sleep(std::time::Duration::from_millis(2000));
        }
    });

    // run the mixer
    mixer.run();
}
