use crate::*;
use core::{fmt::Debug, hash::Hash};

/// sub stream ID for testing purposes.
#[allow(dead_code)]
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaSessionType {
    /// participant's picture
    Camera,
    /// participant's screen share
    ScreenCapture,
}

impl Default for MediaSessionType {
    fn default() -> Self {
        Self::Camera
    }
}

/// Stream ID consisting of one main ID and a sub ID for participants having multiple streams (e.g. Camera and Slides)
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// ID identifying the participant
    pub id: ID,
    /// sub ID identifying the stream of the participant
    pub stream: MediaSessionType,
}

impl<ID> StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug,
{
    pub fn new(id: ID, stream: MediaSessionType) -> Self {
        Self { id, stream }
    }
    pub fn new_main(id: ID) -> Self {
        Self {
            id,
            stream: MediaSessionType::Camera,
        }
    }
}

#[cfg(test)]
impl<ID> From<u32> for StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    fn from(number: u32) -> Self {
        Self {
            id: number.into(),
            stream: MediaSessionType::default(),
        }
    }
}

/// A talk consisting of participants and managing maximum amount of visibles
pub struct Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    #[cfg(test)]
    pub mixer: crate::Mixer<SRC, SINK, StreamId<ID>>,
    #[cfg(not(test))]
    mixer: crate::Mixer<SRC, SINK, StreamId<ID>>,
    max_visibles: Option<usize>,
}

impl<SRC, SINK, ID> Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// create new Talk
    /// # Arguments
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    /// - `max_visibles`: Maximum number of currently visible streams.
    pub fn new(
        resolution: Size,
        sink_params: SINK::Parameters,
        max_visibles: Option<usize>,
    ) -> Result<Self, Error<StreamId<ID>>> {
        let mixer = crate::Mixer::<SRC, SINK, StreamId<ID>>::new(resolution, sink_params)?;
        Ok(Self {
            mixer,
            max_visibles,
        })
    }

    /// add participant with a stream with stream number 0.
    pub fn add_participant<L>(
        &mut self,
        id: StreamId<ID>,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), crate::Error<StreamId<ID>>>
    where
        L: Layout,
    {
        self.mixer.add_stream(id, display_name, params)?;

        let make_visible = if let Some(max_visibles) = self.max_visibles {
            // Show visibles if there is unused space
            self.mixer.visibles.len() < max_visibles
        } else {
            true
        };

        if make_visible {
            debug!(
                "automatically making stream {id:?} visible because there are unused visible ports"
            );
            // get currently visible streams
            let mut visibles = self.mixer.visibles.clone();
            // make new stream visible
            visibles.push(id);
            // update visibles
            self.mixer.set_visibles(&visibles)?;
        }

        // re-layout
        self.mixer.layout::<L>()?;

        Ok(())
    }

    /// add another stream with the given number to an existing participant.
    pub fn add_stream<L>(
        &mut self,
        id: StreamId<ID>,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), crate::Error<StreamId<ID>>>
    where
        L: Layout,
    {
        self.mixer.add_stream(id, display_name, params)
    }

    pub fn remove_stream(
        &mut self,
        remove_id: StreamId<ID>,
    ) -> Result<(), crate::Error<StreamId<ID>>> {
        self.mixer.remove_stream(remove_id)
    }

    pub fn contains_stream(&self, id: &StreamId<ID>) -> bool {
        self.mixer.streams.contains_key(id)
    }

    pub fn push_overlay_text(
        &mut self,
        text: &str,
        text_format: TextFormat,
    ) -> Result<TextOverlay, crate::Error<StreamId<ID>>> {
        let overlay = TextOverlay::new(text, text_format);
        self.push_overlay(Overlay::Text(overlay.clone()))?;
        Ok(overlay)
    }

    fn push_overlay(&mut self, overlay: crate::Overlay) -> Result<(), crate::Error<StreamId<ID>>> {
        self.mixer.push_overlay(overlay)
    }

    pub fn set_speaker(
        &mut self,
        speaker: Option<StreamId<ID>>,
        mode: &SpeakerMode,
    ) -> Result<(), crate::Error<StreamId<ID>>> {
        debug!("set speaker {:?}...", speaker);

        if let Some(speaker) = &speaker {
            let mut visibles = self.mixer.visibles.clone();

            // check if speaker is stream
            if !self.contains_stream(speaker) {
                error!("speaker must be a stream");
            }

            match mode {
                SpeakerMode::FirstShift => {
                    // check if speaker is in visibles
                    match visibles.iter().position(|id| id == speaker) {
                        Some(pos) => {
                            trace!("remove visible at {pos}");
                            // remove speaker from visibles
                            visibles.remove(pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visibles {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                        }
                    }
                    trace!("insert speaker {:?} at 0", *speaker);
                    // insert speaker at first
                    visibles.insert(0, *speaker);
                }
                SpeakerMode::FirstSwap => {
                    // check if speaker is in visibles
                    match visibles.iter().position(|id| id == speaker) {
                        Some(pos) => {
                            trace!("swap visible 0 and {pos}");
                            // swap with previous speaker
                            visibles.swap(0, pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visibles {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                            // insert speaker at first
                            trace!("insert speaker {:?} at 0", *speaker);
                            visibles.insert(0, *speaker);
                        }
                    }
                }
                _ => (),
            }
            self.mixer.set_visibles(&visibles)?;
        }
        Ok(())
    }

    pub fn layout<L>(&self) -> Result<(), crate::Error<StreamId<ID>>>
    where
        L: Layout,
    {
        self.mixer.layout::<L>()
    }

    pub fn get_source(&mut self, id: &StreamId<ID>) -> Option<&mut SRC> {
        if let Some(participant) = self.mixer.streams.get_mut(id) {
            Some(&mut participant.source)
        } else {
            None
        }
    }

    pub fn play(&mut self) {
        self.mixer.play()
    }

    pub fn pause(&mut self) {
        self.mixer.pause()
    }
}

/// Mode in which the current speaker shall be displayed.
#[derive(Debug, Clone)]
pub enum SpeakerMode {
    /// Do not visualize who speaks
    None,
    /// Put the current speaker in front of all others and shift the remaining visible participants down.
    /// If the maximum of visibles is reached and speaker was not visible before the last visible will be shifted out.
    FirstShift,
    /// Put the current speaker in front of all others and if the speaker was visible before swap it with the previous speaker.
    /// If the maximum of visibles is reached and< speaker was not visible before the last visible will be shifted out.
    FirstSwap,
}
