use crate::*;
use core::{fmt::Debug, hash::Hash};

/// A talk consisting of participants and managing maximum amount of visibles
pub struct Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    pub mixer: crate::Mixer<SRC, SINK, ID>,
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
    ) -> Result<Self, Error<ID>> {
        Ok(Self {
            mixer: crate::Mixer::<SRC, SINK, ID>::new(resolution, sink_params)?,
            max_visibles,
        })
    }

    /// add participant
    pub fn add_participant<L>(
        &mut self,
        id: ID,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), crate::Error<ID>>
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

    pub fn remove_stream(&mut self, remove_id: ID) -> Result<(), crate::Error<ID>> {
        self.mixer.remove_stream(remove_id)
    }

    pub fn contains_key(&self, id: &ID) -> bool {
        self.mixer.streams.contains_key(id)
    }

    pub fn push_overlay(&mut self, overlay: crate::Overlay) -> Result<(), crate::Error<ID>> {
        self.mixer.push_overlay(overlay)
    }
    pub fn set_speaker(
        &mut self,
        speaker: Option<ID>,
        mode: &SpeakerMode,
    ) -> Result<(), crate::Error<ID>> {
        debug!("set speaker {:?}...", speaker);

        if let Some(speaker) = &speaker {
            let mut visibles = self.mixer.visibles.clone();

            // check if speaker is stream
            if !self.contains_key(speaker) {
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

    pub fn layout<L>(&self) -> Result<(), crate::Error<ID>>
    where
        L: Layout,
    {
        self.mixer.layout::<L>()
    }

    pub fn get_source(&mut self, id: &ID) -> Option<&mut SRC> {
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
