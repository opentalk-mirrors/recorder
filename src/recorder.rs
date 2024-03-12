// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{bail, Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{
    MatroskaSink, MediaDescriptor, Mp4Parameters, Mp4Sink, RTMPParameters, RTMPSink, SystemSink,
    WebRtcSource, WebRtcSourceParams,
};
use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};
use futures::Stream;
use log::error;
use std::{io, path::Path, sync::Arc};
use tempfile::TempDir;
use tokio::{
    fs::File,
    io::{AsyncRead, ReadBuf},
    sync::{mpsc, watch},
    task::{spawn_blocking, JoinHandle},
};
use types::{
    core::ParticipantId,
    signaling::media::{MediaSessionState, MediaSessionType},
};

use crate::{
    http::HttpClient,
    rmq::InitializeRecording,
    settings::{RecorderSettings, RecorderSink, Settings},
    signaling::{Event, Signaling, TrickleCandidate},
};

// TODO; make this configurable
pub const MAX_VISIBLES: usize = 8;

type Mixer = compositor::Mixer<WebRtcSource>;

#[derive(Clone, Debug)]
pub struct Recorder {
    pub settings: Arc<Settings>,
    pub http_client: Arc<HttpClient>,
    pub shutdown: watch::Receiver<bool>,
}

impl Recorder {
    /// This constructor is used by the integration tests to mock data.
    pub fn new(
        settings: Settings,
        http_client: HttpClient,
        shutdown: watch::Receiver<bool>,
    ) -> Self {
        Self {
            settings: Arc::new(settings),
            http_client: Arc::new(http_client),
            shutdown,
        }
    }

    pub async fn spawn_session(
        &self,
        command: InitializeRecording,
    ) -> Result<JoinHandle<Result<()>>> {
        let context = Arc::new(self.clone());
        log::debug!("Start Recording session {command:?}");
        let mut session = RecordingSession::create(context, command)
            .await
            .context("recording session failed to start")?;

        let recording_task = tokio::spawn(async move {
            if let Err(ref recording_err) = session.run().await {
                error!(
                    "recording session failed but trying upload anyway {:?}",
                    recording_err
                );
            };
            session.upload().await.context("recording upload failed")?;

            Ok(())
        });

        Ok(recording_task)
    }

    pub async fn upload(&self, room_id: &str, recording_path: &Path) -> Result<()> {
        let file = File::open(recording_path).await?;

        log::debug!(
            "upload mp4 file '{:?}' for room: {}",
            recording_path,
            room_id
        );

        self.http_client
            .upload_render(&self.settings.controller, room_id, FileReadStream { file })
            .await
    }
}

#[derive(Debug)]
pub struct RecordingSession {
    service_context: Arc<Recorder>,

    signaling: Signaling,

    room_id: String,
    temp_dir: TempDir,

    mixer: Mixer,

    candidate_receiver: mpsc::Receiver<(MediaDescriptor, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(MediaDescriptor, u32, Option<String>)>,

    done: bool,
}

impl RecordingSession {
    /// This constructor is used by the integration tests to mock data.
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        service_context: Arc<Recorder>,
        signaling: Signaling,
        room_id: String,
        temp_dir: TempDir,
        mixer: Mixer,
        candidate_receiver: mpsc::Receiver<(MediaDescriptor, u32, Option<String>)>,
        candidate_sender: mpsc::Sender<(MediaDescriptor, u32, Option<String>)>,
        done: bool,
    ) -> Self {
        Self {
            service_context,
            signaling,
            room_id,
            temp_dir,
            mixer,
            candidate_receiver,
            candidate_sender,
            done,
        }
    }

    pub async fn create(
        service_context: Arc<Recorder>,
        command: InitializeRecording,
    ) -> Result<RecordingSession> {
        let signaling = Signaling::connect(
            service_context.http_client.as_ref(),
            &service_context.settings.controller,
            &command.room,
        )
        .await?;

        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("out.mp4");

        let (candidate_sender, candidate_receiver) = mpsc::channel(12);

        let recorder_settings = service_context.settings.recorder.as_ref();
        let recorder_sinks = recorder_settings
            .unwrap_or(&RecorderSettings { sinks: vec![] })
            .sinks
            .clone();

        let mut mixer = Mixer::create(
            None,
            compositor::Size::FHD,
            compositor::layout::Speaker::default(),
            MAX_VISIBLES,
            true,
        )?;

        for (index, sink) in recorder_sinks.into_iter().enumerate() {
            let tag = match sink {
                RecorderSink::Display => "Display",
                RecorderSink::Matroska(_) => "Matroska",
                RecorderSink::Rtmp(_) => "RTMP",
            };
            let name = format!("{tag}-Sink-{index}");
            match sink {
                RecorderSink::Display => {
                    mixer
                        .link_sink(
                            name.as_str(),
                            SystemSink::create(name.as_str(), true)
                                .context("DisplaySink could not created")?,
                        )
                        .context("unable to link sink to mixer")?;
                }
                RecorderSink::Matroska(matroska_parameters) => {
                    mixer
                        .link_sink(
                            name.as_str(),
                            MatroskaSink::create(name.as_str(), &matroska_parameters)
                                .context("MatroskaSink could not created")?,
                        )
                        .context("unable to link sink to mixer")?;
                }

                RecorderSink::Rtmp(rtmp_parameters) => {
                    mixer
                        .link_sink(
                            name.as_str(),
                            RTMPSink::create(
                                name.as_str(),
                                RTMPParameters {
                                    location: rtmp_parameters
                                        .location
                                        .replace("$room", &command.room),
                                    ..rtmp_parameters.clone()
                                },
                            )
                            .context("RTMPSink could not created")?,
                        )
                        .context("unable to link sink to mixer")?;
                }
            }
        }

        mixer
            .link_sink(
                "mp4",
                Mp4Sink::create(
                    "MP4-Sink",
                    &Mp4Parameters {
                        file_path: file_path
                            .to_str()
                            .context("failed to convert MP4 file path into string")?
                            .into(),
                        name: "Recording",
                    },
                )
                .context("MP4-Sink could not created")?,
            )
            .context("unable to link sink to mixer")?;

        Ok(Self {
            service_context,
            signaling,
            room_id: command.room,
            temp_dir,
            mixer,
            candidate_receiver,
            candidate_sender,
            done: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut shutdown_rx = self.service_context.shutdown.clone();

        while !self.done {
            tokio::select! {
                event = self.signaling.run() => {
                    let signaling_msg = event.context("signaling error")?;
                    log::trace!("signaling_event {:?}", signaling_msg);
                    self.handle_signaling_event(signaling_msg).await?;
                }
                maybe_candidate = self.candidate_receiver.recv() => {
                    let Some((descriptor, mline, candidate)) = maybe_candidate else {
                        bail!("no candidate pair found");
                    };
                    self.handle_candidate(descriptor, mline, candidate).await?;
                }
                result = shutdown_rx.changed() => {
                    if result.is_err() {
                        return result.context("failed to listen to shutdown signal");
                    }
                    if *shutdown_rx.borrow() {
                        self.done = true;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn subscribe(
        &mut self,
        descriptor: MediaDescriptor,
        display_name: &str,
        media_state: MediaSessionState,
    ) -> Result<()> {
        self.mixer.add_stream(
            descriptor,
            display_name.to_owned(),
            stream_params(descriptor, self.candidate_sender.clone()),
            media_state,
        )?;
        self.signaling.start_subscribe(descriptor).await?;

        if media_state.video {
            self.mixer
                .show_stream(descriptor)
                .context("unable to show stream for descriptor '{descriptor}'")?;
        }

        Ok(())
    }

    // TODO: This makes no sense at the current state, docs will be created after some major refactoring.
    #[allow(clippy::too_many_lines)]
    async fn handle_signaling_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::JoinSuccess(_id, title) => {
                // find all active media streams

                let available_media_streams: Vec<(
                    ParticipantId,
                    String,
                    MediaSessionType,
                    MediaSessionState,
                )> = self
                    .signaling
                    .participants()
                    .iter()
                    .flat_map(|(id, participant_state)| {
                        media_types().filter_map(|media_type| {
                            participant_state.publishes(media_type).map(|media_state| {
                                (
                                    *id,
                                    participant_state.display_name.clone(),
                                    media_type,
                                    media_state,
                                )
                            })
                        })
                    })
                    .collect();

                for (id, display_name, media_type, media_state) in available_media_streams {
                    log::debug!("JoinSuccess: subscribe stream of {id} {media_type}");
                    let descriptor = MediaDescriptor {
                        participant_id: id,
                        media_type,
                    };
                    self.subscribe(descriptor, &display_name, media_state)
                        .await?;
                }

                self.mixer.set_title(title.as_str());
            }

            Event::ParticipantJoined(id) => {
                log::debug!("Event::ParticipantJoined");

                let participant_state = self.signaling.participant(&id)?.clone();
                let available_media_streams = media_types().filter_map(|media_type| {
                    participant_state
                        .publishes(media_type)
                        .map(|media_state| (media_type, media_state))
                });

                for (media_type, media_state) in available_media_streams {
                    log::debug!("Join: subscribe stream of {id} {media_type}");
                    let descriptor = MediaDescriptor {
                        participant_id: id,
                        media_type,
                    };
                    self.subscribe(descriptor, &participant_state.display_name, media_state)
                        .await?;
                }
            }
            Event::ParticipantUpdated(id) => {
                log::debug!("Event::ParticipantUpdated");
                let participant_state = self.signaling.participant(&id)?.clone();

                for media_type in media_types() {
                    let is_subscribed = self.mixer.contains_stream(MediaDescriptor {
                        participant_id: id,
                        media_type,
                    });
                    let media_state = participant_state.publishes(media_type);

                    if !is_subscribed {
                        if let Some(media_state) = media_state {
                            log::debug!("Update: subscribe stream of {id} {media_type}");
                            let descriptor = MediaDescriptor {
                                participant_id: id,
                                media_type,
                            };
                            self.subscribe(
                                descriptor,
                                &participant_state.display_name,
                                media_state,
                            )
                            .await?;
                        }
                    } else if media_state.is_none() {
                        log::debug!("Update: unsubscribe stream of {id} {media_type}");
                        self.mixer.remove_stream(MediaDescriptor {
                            participant_id: id,
                            media_type,
                        })?;
                    } else if let Some(media_state) = media_state {
                        log::debug!(
                            "Update: update status of stream of {id} {media_type} to {media_state}"
                        );
                        self.mixer.set_status(
                            MediaDescriptor {
                                participant_id: id,
                                media_type,
                            },
                            media_state,
                        )?;
                    } else {
                        log::trace!(
                            "ignore update for {id}: media_state ({media_state:?}) == is_subscribed ({is_subscribed})"
                        );
                        return Ok(());
                    }
                }

                return Ok(());
            }
            Event::ParticipantLeft(id) => {
                log::debug!("Event::ParticipantLeft");
                for media_type in media_types() {
                    if self.mixer.contains_stream(MediaDescriptor {
                        participant_id: id,
                        media_type,
                    }) {
                        self.mixer.remove_stream(MediaDescriptor {
                            participant_id: id,
                            media_type,
                        })?;
                    }
                }
                if self.signaling.participants().is_empty() {
                    self.done = true;
                    log::debug!("Last participant left the session. Stop recording.");
                } else {
                    log::trace!(
                        "{} remaining participants : {:?}",
                        self.signaling.participants().len(),
                        self.signaling.participants().keys()
                    );
                }
            }
            Event::SdpOffer(descriptor, offer) => {
                log::debug!("Event::SdpOffer");
                if let Some(source) = self.mixer.get_source(descriptor) {
                    let answer = source.receive_offer(offer).await?;
                    self.signaling.send_answer(descriptor, answer).await?;
                }
            }
            Event::SdpCandidate(descriptor, candidate) => {
                log::debug!("Event::SdpCandidate");
                if let Some(source) = self.mixer.get_source(descriptor) {
                    source
                        .receive_candidate(candidate.sdp_m_line_index as u32, &candidate.candidate);
                }
            }
            Event::SdpEndOfCandidates(descriptor) => {
                log::debug!("Event::SdpEndOfCandidates");
                let participant_state = self.signaling.participant(&descriptor.participant_id)?;

                if participant_state.publishes(descriptor.media_type).is_none() {
                    bail!(
                        "EndOfCandidates message for {:?} with no media stream",
                        descriptor
                    );
                }
                let Some(source) = self.mixer.get_source(descriptor) else {
                    bail!(
                        "EndOfCandidates message for {:?} with no connection setup",
                        descriptor
                    );
                };

                source.receive_end_of_candidates(0);
            }
            Event::FocusUpdate(focus_change) => {
                log::debug!("Event::FocusUpdate");
                log::debug!("Set active speaker to {:?}", focus_change);
                if let Some(speaker) = focus_change {
                    self.mixer
                        .set_speaker(speaker)
                        .context("unable to set speaker for '{speaker}'")?;
                }
            }
            Event::MediaConnectionError(error) => {
                log::debug!("Event::MediaConnectionError");
                log::warn!("Skipping media connection error: {:?}", error);
            }
            Event::Close => self.done = true,
        }

        Ok(())
    }

    /// Handle SDP candidates generated by us
    async fn handle_candidate(
        &mut self,
        descriptor: MediaDescriptor,
        mline: u32,
        candidate: Option<String>,
    ) -> Result<()> {
        if let Some(candidate) = candidate {
            self.signaling
                .send_candidate(
                    descriptor,
                    TrickleCandidate {
                        candidate: candidate.clone(),
                        sdp_m_line_index: u64::from(mline),
                    },
                )
                .await
        } else {
            self.signaling.send_end_of_candidates(descriptor).await
        }
    }

    async fn upload(self) -> Result<()> {
        let mixer = self.mixer;
        spawn_blocking(move || drop(mixer)).await?;

        let recording_path = self.temp_dir.path().join("out.mp4");

        let Err(upload_err) = self
            .service_context
            .upload(&self.room_id, recording_path.as_ref())
            .await
        else {
            log::debug!("Finished uploading recording for room '{}'", &self.room_id);
            return Ok(());
        };

        let dump_name = "DUMP.mp4";
        error!(
            "upload of file {:?} failed. Saving output in {dump_name}.",
            recording_path
        );
        tokio::fs::copy(recording_path, dump_name).await?;

        Err(upload_err)
    }
}

fn stream_params(
    id: MediaDescriptor,
    sender: mpsc::Sender<(MediaDescriptor, u32, Option<String>)>,
) -> WebRtcSourceParams {
    WebRtcSourceParams::new(true).on_ice_candidate(move |mline, candidate| {
        let _ = sender.blocking_send((id, mline, candidate));
    })
}

pin_project_lite::pin_project! {
    /// Helper struct which reads an opened file and returns chunks of up to 8kb as Stream
    struct FileReadStream {
        #[pin]
        file: tokio::fs::File,
    }
}

impl Stream for FileReadStream {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let mut buf = [0u8; 8192];
        let mut read_buf = ReadBuf::new(&mut buf);
        ready!(this.file.poll_read(cx, &mut read_buf))?;

        let buffer = read_buf.filled();
        if buffer.is_empty() {
            return Poll::Ready(None);
        }

        Poll::Ready(Some(Ok(Bytes::copy_from_slice(buffer))))
    }
}

#[must_use]
fn media_types() -> impl DoubleEndedIterator<Item = MediaSessionType> {
    [MediaSessionType::Screen, MediaSessionType::Video].into_iter()
}
