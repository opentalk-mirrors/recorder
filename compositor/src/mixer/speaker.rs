use super::helpers::*;
use super::*;
use gst::prelude::*;
use gstreamer as gst;
use layout::*;
use mixer::*;

pub type SourceFactory = fn(&gst::Pipeline, &str, &Size) -> (gst::GhostPad, gst::GhostPad);

impl Layout {
    /// create a layout where the viewers are vertically distributed at the right side
    /// of the speaker and remaining space is used to display a title and 'who's speaking'
    /// # Arguments
    /// - `width` : width of the output picture in pixels
    /// - `height` : height of the output picture in pixels
    /// - `num_viewers` : number of viewers to display
    /// # Return
    /// Returns a `Layout` instance you can use to call `Mixer::new_speaker()`.
    pub fn new_speaker_vertical(resolution: &Size, num_viewers: usize) -> Self {
        // calculate views' sizes so that `num_viewers` viewers fit within picture height
        let ratio = resolution.width as f64 / resolution.height as f64;
        let viewers_height = if num_viewers > 0 {
            resolution.height / num_viewers
        } else {
            0
        };
        let viewers_width = (viewers_height as f64 * ratio) as usize;
        // calculate speaker's size by taking place beside the viewers
        let speaker_height = resolution.height - viewers_height;
        let speaker_width = (speaker_height as f64 * ratio) as usize;
        // calculate layout
        Self {
            // overall picture size
            size: Size {
                width: resolution.width,
                height: resolution.height,
            },
            // calculate viewers' positions
            viewers_positions: match num_viewers {
                // place one viewer centered beside the speaker
                1 => vec![Position {
                    x: resolution.width as i64 / 2,
                    y: resolution.height as i64 / 4,
                }],
                // otherwise arrange viewers at the right side of the picture
                _ => (0..num_viewers)
                    .into_iter()
                    .map(|n| Position {
                        x: speaker_width as i64,
                        y: (viewers_height * n) as i64,
                    })
                    .collect(),
            },
            // calculate viewers' size
            viewers_size: match num_viewers {
                // fit one viewer beside the speaker
                1 => Size {
                    width: resolution.width / 2,
                    height: resolution.height / 2,
                },
                // otherwise use viewers' size
                _ => Size {
                    width: viewers_width,
                    height: viewers_height,
                },
            },
            // calculate speaker's position
            speaker_position: match num_viewers {
                // place speaker beside single viewer
                1 => Position {
                    x: 0,
                    y: resolution.height as i64 / 4,
                },
                // place speaker beside the viewer arrangement and leave space at top
                _ => Position {
                    x: 0,
                    y: resolution.height as i64 - speaker_height as i64,
                },
            },
            // calculate speaker's size
            speaker_size: match num_viewers {
                // fit speaker beside single viewer
                1 => Size {
                    width: resolution.width / 2,
                    height: resolution.height / 2,
                },
                // fit speaker beside the viewer arrangement
                _ => Size {
                    width: speaker_width,
                    height: speaker_height,
                },
            },
            // align the title text
            title_alignment: Alignment {
                horizontal: "left",
                vertical: "top",
            },
            // place the title at the top left corner
            title_position: Position { x: 0, y: 0 },
            // align the "who's speaking" text
            speaking_alignment: Alignment {
                horizontal: "left",
                vertical: match num_viewers {
                    // put to the bottom when none or only one viewer is available
                    0 | 1 => "bottom",
                    // otherwise we center it within the title area
                    _ => "center",
                },
            },
            // place "who's speaking" text
            speaking_position: Position {
                x: 0,
                y: match num_viewers {
                    // straight at the bottom (see `speaking_alignment`)
                    0 | 1 => 0,
                    // center vertically within title area
                    _ => -(speaker_height as i64 / 2),
                },
            },
            // align clock display
            clock_alignment: Alignment {
                horizontal: "right",
                vertical: "bottom",
            },
            // place clock display
            clock_position: Position {
                x: match num_viewers {
                    // right within whole picture
                    0 | 1 => 0,
                    // right within title area
                    _ => -(viewers_width as i64),
                },
                y: match num_viewers {
                    // bottom of the whole picture
                    0 => 0,
                    // bottom within title area
                    _ => -(speaker_height as i64),
                },
            },
        }
    }
}

impl Mixer {
    /// create a mixer from a given layout
    /// # Arguments
    /// - `pipeline` : the pipeline to add the speaker view into
    /// - `layout` : Layout of speaker and viewers
    /// - `factory` : factory to create A/V sources
    /// # Returns
    /// Returns two `GhostPad` instances: 1st for video and 2nd for audio
    pub fn new_speaker(
        pipeline: &gst::Pipeline,
        layout: &Layout,
        factory: SourceFactory,
    ) -> (gst::GhostPad, gst::GhostPad) {
        // create and link video mixer
        let (video_mixer_pad, video_participants_pads) = Self::create_video(&pipeline, &layout);

        // create and link audio mixer
        let (audio_mixer_pad, audio_participants_pads) = Self::create_audio(&pipeline, &layout);

        // create and link speaker's source
        let (video_source_speaker, audio_source_speaker) =
            factory(&pipeline, "video-source-speaker", &layout.speaker_size);

        // link video to mixer
        video_source_speaker
            .link(&video_participants_pads[0])
            .unwrap();

        // link audio to mixer
        audio_source_speaker
            .link(&audio_participants_pads[0])
            .unwrap();

        // create and link viewers' sources
        for n in 1..video_participants_pads.len() {
            // create speaker source
            let (video_source_viewer, audio_source_viewer) = factory(
                &pipeline,
                &format!("video-source-viewer{n}"),
                &layout.viewers_size,
            );
            // link video to video mixer
            video_source_viewer
                .link(&video_participants_pads[n])
                .unwrap();
            // link audio to audio mixer
            audio_source_viewer
                .link(&audio_participants_pads[n])
                .unwrap();
        }
        // link to mixer
        (video_mixer_pad, audio_mixer_pad)
    }

    /// create an video mixer from a given layout
    /// # Arguments
    /// - `pipeline` : the pipeline to add the video mixer into
    /// - `layout` : Layout of speaker and viewers
    /// # Returns
    /// Returns two `GhostPad` instances: 1st for video and 2nd for audio
    #[allow(dead_code)]
    fn create_video(
        pipeline: &gst::Pipeline,
        layout: &Layout,
    ) -> (gst::GhostPad, Vec<gst::GhostPad>) {
        // prepare compositor input sinks
        let sink_pads = (0..layout.num_viewers())
            .into_iter()
            .map(|n| {
                format!(
                    "\n            sink_{n}::xpos={xpos}
                    sink_{n}::ypos={ypos}
                    sink_{n}::width={width}
                    sink_{n}::height={height}",
                    xpos = layout.viewers_positions[n].x,
                    ypos = layout.viewers_positions[n].y,
                    width = layout.viewers_size.width,
                    height = layout.viewers_size.height,
                    n = n + 1
                )
            })
            .collect::<Vec<String>>()
            .join("");

        // prepare a bin with the compositor
        let bin = format!(
            r#"
                compositor
                    name=video-mixer
                    background=black
                    sink_0::xpos={speaker_x}
                    sink_0::ypos={speaker_y}
                    sink_0::width={speaker_width}
                    sink_0::height={speaker_height}{sink_pads}
                ! clockoverlay
                    name=clock
                    font-desc="Sans, 14"
                    time-format="%x %X %Z"
                    halignment={clock_align_h}
                    valignment={clock_align_v}
                    line-alignment={clock_align_h}
                    deltax={clock_x}
                    deltay={clock_y}
                    xpad=10
                    ypad=2
                    color=0xffffffff
                ! textoverlay
                    name=title
                    font-desc="Sans, 16"
                    halignment={title_align_h}
                    valignment={title_align_v}
                    line-alignment={title_align_h}
                    deltax={title_x}
                    deltay={title_y}
                    xpad=10
                    ypad=2
                    color=0xffffffff
                ! textoverlay
                    name=speaking
                    font-desc="Sans, 16"
                    halignment={speaking_align_h}
                    valignment={speaking_align_v}
                    line-alignment={speaking_align_h}
                    deltax={speaking_x}
                    deltay={speaking_y}
                    xpad=10
                    ypad=2
                    color=0xffffffff
                ! video/x-raw,width={width},height={height}
                ! queue
                    name=video-mixer-output
                "#,
            width = layout.size.width,
            height = layout.size.height,
            speaker_x = layout.speaker_position.x,
            speaker_y = layout.speaker_position.y,
            speaker_width = layout.speaker_size.width,
            speaker_height = layout.speaker_size.height,
            title_align_h = layout.title_alignment.horizontal,
            title_align_v = layout.title_alignment.vertical,
            title_x = layout.title_position.x,
            title_y = layout.title_position.y,
            speaking_align_h = layout.speaking_alignment.horizontal,
            speaking_align_v = layout.speaking_alignment.vertical,
            speaking_x = layout.speaking_position.x,
            speaking_y = layout.speaking_position.y,
            clock_align_h = layout.clock_alignment.horizontal,
            clock_align_v = layout.clock_alignment.vertical,
            clock_x = layout.clock_position.x,
            clock_y = layout.clock_position.y,
        );

        // parse bin and add it to the pipeline
        info!("parsing video mixer bin:\n{bin}");
        let bin = gst::parse_bin_from_description(&bin, false).unwrap();
        pipeline.add(&bin).unwrap();

        // link our internal sink to a ghost pad at the bin's outside
        let mixer_pad = link_bin_ghost_pad(&bin, "video-mixer-output", "src");
        let participants_pads: Vec<gst::GhostPad> = (0..layout.num_viewers() + 1)
            .into_iter()
            .map(|n| link_bin_add_ghost_pad(&bin, "video-mixer", &format!("sink_{n}")))
            .collect();

        // return pads of interest
        (mixer_pad, participants_pads)
    }
    /// create an audio mixer from a given layout
    /// # Arguments
    /// - `pipeline` : the pipeline to add the audio mixer into
    /// - `layout` : Layout of speaker and viewers
    /// # Returns
    /// Returns two `GhostPad` instances: 1st for video and 2nd for audio
    #[allow(dead_code)]
    fn create_audio(
        pipeline: &gst::Pipeline,
        layout: &Layout,
    ) -> (gst::GhostPad, Vec<gst::GhostPad>) {
        // prepare a bin with the compositor
        let bin = format!(
            r#"
                audiomixer
                    name=audio-mixer
                ! queue
                    name=audio-mixer-output
                "#,
        );

        // parse bin and add it to the pipeline
        info!("parsing audio mixer bin:\n{bin}");
        let bin = gst::parse_bin_from_description(&bin, false).unwrap();

        pipeline.add(&bin).unwrap();

        // link our internal sink to a ghost pad at the bin's outside
        let mixer_pad = link_bin_ghost_pad(&bin, "audio-mixer-output", "src");
        let participants_pads: Vec<gst::GhostPad> = (0..layout.num_viewers() + 1)
            .into_iter()
            .map(|n| link_bin_add_ghost_pad(&bin, "audio-mixer", &format!("sink_{n}")))
            .collect();

        (mixer_pad, participants_pads)
    }
}
