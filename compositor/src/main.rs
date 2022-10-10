mod error;
mod layout;
mod mixer;
#[cfg(test)]
mod tests;

extern crate clap;
#[macro_use]
extern crate log;

use clap::Parser;
use gstreamer as gst;
use layout::*;
use mixer::*;

#[derive(Debug, Clone, clap::ValueEnum)]
enum LayoutT {
    Speaker,
    Grid,
}

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
    /// select layout
    #[clap(short, long, value_enum, default_value = "grid")]
    layout: LayoutT,
    /// use test sources
    #[clap(short, long)]
    test: bool,
}

fn main() {
    // initialize logger
    env_logger::init();

    // parse command line arguments
    let args = Arguments::parse();

    match args.layout {
        LayoutT::Speaker => run::<Speaker>(args),
        LayoutT::Grid => run::<Grid>(args),
    }
}
fn run<L>(args: Arguments)
where
    L: Layout + Clone,
{
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
    let mut mixer =
        Mixer::<L, TestSource>::new::<DisplaySink>(resolution, args.participants, args.visibles)
            .unwrap();

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer.generate_dot_file("pipeline-initial");
    }

    mixer.play();

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer.generate_dot_file("pipeline-first-playing");
    }

    // shall we create DOT file of mixer's pipeline?
    if args.dot {
        // must set this to work
        mixer.generate_dot_file("pipeline");
    }

    let run_fn = mixer.run();

    // start thread which continuously switches speaking text
    std::thread::spawn({
        move || {
            // clone a mixer instance for the thread
            // initially set title
            mixer.set_title("Some very important meeting");

            // add participant names
            let names: Vec<String> = (0..args.participants)
                .enumerate()
                .map(|(n, _)| format!("Participant {n}"))
                .collect();

            mixer.pause();
            for participant in &names {
                mixer
                    .add_participant(
                        participant.clone(),
                        (
                            participant.clone(),
                            "smpte",
                            Size {
                                width: 1240,
                                height: 720,
                            },
                        ),
                    )
                    .unwrap();
            }
            mixer.play();

            let mut i: usize = 0;
            let mut m: isize = 0;
            let mut step: isize = 1;
            let mut removed = Vec::new();
            loop {
                trace!("------------------------ {i} ({m} visibles) ------------------------");
                // pause before changing the scene
                mixer.pause();

                // select participant names for visibility
                let names: Vec<String> = (0..m)
                    .enumerate()
                    .map(|(n, _)| format!("Participant {n}"))
                    .filter(|s| !removed.contains(s))
                    .collect();
                mixer.set_visibles(&names).unwrap();

                /*                if let Some(last) = names.last() {
                                    mixer.remove_participants(&[last.to_string()]).unwrap();
                                    removed.push(last.to_string());
                                }
                */
                // continuously set who's speaking
                if !names.is_empty() {
                    mixer.set_speaking(&names[i % names.len()]);
                    // enumerate names
                    i += 1;
                }
                mixer.layout().unwrap();

                // play after changing the scene
                mixer.play();

                // shall we create DOT file of mixer's pipeline?
                if args.dot {
                    // must set this to work
                    mixer.generate_dot_file(&format!("pipeline-{i}-{m}"));
                }

                // enumerate number of visibles up and down in a loop
                if m >= args.visibles as isize {
                    step = -1;
                } else if m == 0 {
                    step = 1;
                }
                m += step;
                // let it happen
                std::thread::sleep_ms(500);
            }
        }
    });

    // run the mixer
    run_fn();
}
