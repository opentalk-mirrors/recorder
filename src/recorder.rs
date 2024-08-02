// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Debug,
    io,
    path::Path,
    sync::Arc,
};

use anyhow::{bail, Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{
    MediaDescriptor, RTMPParameters, RTMPSink, SystemSink, WebMParameters, WebMSink, WebRtcSource,
    WebRtcSourceParams,
};
use futures::Stream;
use log::error;
use tempfile::TempDir;
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncRead, ReadBuf},
    sync::{mpsc, watch},
    task::JoinHandle,
};
use types::{
    core::{ParticipantId, StreamingTargetId},
    signaling::{
        media::{MediaSessionState, MediaSessionType},
        recording::{
            state::{RecorderStreamInfo, StreamingTarget},
            StreamErrorReason, StreamStatus,
        },
    },
};

use crate::{
    http::{FileExtension, HttpClient},
    rmq::InitializeRecording,
    settings::{RecorderSink, Settings},
    signaling::{Event, Signaling, TrickleCandidate},
};

// TODO; make this configurable
pub const MAX_VISIBLES: usize = 8;

const TEMP_RECORDING_NAME: &str = "recording.webm";

type Mixer = compositor::Mixer<WebRtcSource>;

#[derive(Clone, Debug)]
pub enum RecorderStreamKind {
    Recording { file_name: String },
    Streaming { target: StreamingTarget },
}

#[derive(Clone, Debug)]
pub struct RecorderStreamStatus {
    pub state: StreamStatus,
    pub kind: RecorderStreamKind,
}

impl RecorderStreamStatus {
    #[must_use]
    pub fn stream_running(&self) -> bool {
        self.state == StreamStatus::Active
            || self.state == StreamStatus::Paused
            || self.state == StreamStatus::Starting
    }
}

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
                    "recording session failed but trying upload anyway:\n{:?}",
                    recording_err
                );
            };

            Ok(())
        });

        Ok(recording_task)
    }

    pub async fn upload(&self, room_id: &str, recording_path: &Path) -> Result<()> {
        let file = File::open(recording_path).await?;

        log::debug!("upload file '{:?}' for room: {}", recording_path, room_id);

        self.http_client
            .upload_render(
                &self.settings.controller,
                room_id,
                FileExtension::webm(),
                FileReadStream { file },
            )
            .await
    }
}

#[derive(Debug)]
pub struct RecordingSession {
    service_context: Arc<Recorder>,

    signaling: Signaling,

    room_id: String,
    participant_id: Option<ParticipantId>,

    temp_dir: TempDir,

    mixer: Mixer,

    streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamStatus>,

    candidate_receiver: mpsc::Receiver<(MediaDescriptor, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(MediaDescriptor, u32, Option<String>)>,

    done: bool,

    configurations: HashMap<MediaDescriptor, bool>,
}

#[derive(Debug, Error)]
pub enum RecordingSessionError {
    #[error("Stream '{0}' is already running")]
    AlreadyRunning(StreamingTargetId),
    #[error("Stream '{0}' not found")]
    NotFound(StreamingTargetId),
    #[error("Stream '{0}' has no location")]
    NoLocation(StreamingTargetId),
    #[error("Stream '{0}' is not running")]
    NotRunning(StreamingTargetId),

    #[error("Start livestream failed, reason: {0}")]
    StartLivestream(anyhow::Error),
    #[error("Stop livestream failed, reason: {0}")]
    StopLivestream(anyhow::Error),

    #[error("Start recording failed, reason: {0}")]
    StartRecording(anyhow::Error),
    #[error("Stop recording failed, reason: {0}")]
    StopRecording(anyhow::Error),
    #[error("Upload recording failed, reason: {0}")]
    UploadRecording(anyhow::Error),
}

impl From<RecordingSessionError> for StreamErrorReason {
    fn from(value: RecordingSessionError) -> Self {
        let code = match value {
            RecordingSessionError::AlreadyRunning(_) => "already_running".to_owned(),
            RecordingSessionError::NotFound(_) => "not_found".to_owned(),
            RecordingSessionError::NotRunning(_) => "not_running".to_owned(),
            RecordingSessionError::NoLocation(_) => "no_location".to_owned(),

            RecordingSessionError::StartLivestream(_) => "start_livestream".to_owned(),
            RecordingSessionError::StopLivestream(_) => "stop_livestream".to_owned(),

            RecordingSessionError::StartRecording(_) => "start_recording".to_owned(),
            RecordingSessionError::StopRecording(_) => "stop_recording".to_owned(),
            RecordingSessionError::UploadRecording(_) => "upload_recording".to_owned(),
        };

        Self {
            code,
            message: value.to_string(),
        }
    }
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
        streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamStatus>,
        candidate_receiver: mpsc::Receiver<(MediaDescriptor, u32, Option<String>)>,
        candidate_sender: mpsc::Sender<(MediaDescriptor, u32, Option<String>)>,
        done: bool,
    ) -> Self {
        Self {
            service_context,
            signaling,
            room_id,
            participant_id: None,
            temp_dir,
            mixer,
            streaming_targets,
            candidate_receiver,
            candidate_sender,
            done,
            configurations: HashMap::new(),
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
            &command.breakout,
        )
        .await?;

        let temp_dir = TempDir::new()?;

        let (candidate_sender, candidate_receiver) = mpsc::channel(12);

        let recorder_settings = service_context
            .settings
            .recorder
            .clone()
            .unwrap_or_default();
        let recorder_sinks = recorder_settings.sinks.clone();

        let mut mixer = Mixer::create(
            compositor::Size::FHD,
            compositor::layout::Speaker::default(),
            MAX_VISIBLES,
            true,
            &recorder_settings.clock_format,
        )?;

        for (index, sink) in recorder_sinks.into_iter().enumerate() {
            let tag = match sink {
                RecorderSink::Display => "Display",
                RecorderSink::WebM(_) => "WebM",
                RecorderSink::Rtmp(_) => "RTMP",
            };
            let name = format!("{tag}-Sink-{index}");
            match sink {
                RecorderSink::Display => {
                    mixer.link_sink(
                        name.as_str(),
                        SystemSink::create(name.as_str(), true)
                            .context("DisplaySink could not created")?,
                    )?;
                }
                RecorderSink::WebM(webm_parameters) => {
                    mixer.link_sink(
                        name.as_str(),
                        WebMSink::create(name.as_str(), &webm_parameters)
                            .context("WebMSink could not created")?,
                    )?;
                }

                RecorderSink::Rtmp(rtmp_parameters) => {
                    mixer.link_sink(
                        name.as_str(),
                        RTMPSink::create(
                            name.as_str(),
                            RTMPParameters {
                                location: rtmp_parameters.location.replace("$room", &command.room),
                                ..rtmp_parameters.clone()
                            },
                        )
                        .context("RTMPSink could not created")?,
                    )?;
                }
            }
        }

        Ok(Self {
            service_context,
            signaling,
            room_id: command.room,
            participant_id: None,
            temp_dir,
            mixer,
            streaming_targets: BTreeMap::new(),
            candidate_receiver,
            candidate_sender,
            done: false,
            configurations: HashMap::new(),
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

        // The streaming targets are per session
        // therefore making sure we're in the right context isn't necessary here.
        log::debug!("Recorder is done, attempting to upload remaining streams...");
        let cloned_stream = self.streaming_targets.clone();
        for (stream_target_id, _) in cloned_stream
            .iter()
            .filter(|(_, status)| RecorderStreamStatus::stream_running(status))
        {
            self.stop_stream(*stream_target_id).await?;
        }

        Ok(())
    }

    pub fn start_recording(mixer: &mut Mixer, temp_dir: &TempDir, file_name: &str) -> Result<()> {
        let file_path = temp_dir.path().join(file_name);
        mixer.link_sink(
            "recording",
            WebMSink::create(
                "WebM-Sink",
                &WebMParameters {
                    path: file_path
                        .to_str()
                        .context("failed to convert WebM file path into string")?
                        .into(),
                },
            )
            .context("WebM-Sink could not created")?,
        )?;

        Ok(())
    }

    fn start_stream(
        &mut self,
        id: StreamingTargetId,
    ) -> Result<StreamStatus, RecordingSessionError> {
        log::trace!("start_stream, id: {id:?}");
        let Some(stream) = self.streaming_targets.get_mut(&id) else {
            return Err(RecordingSessionError::NotFound(id));
        };
        if stream.state == StreamStatus::Active {
            return Err(RecordingSessionError::AlreadyRunning(id));
        }
        let new_state = match &stream.kind {
            RecorderStreamKind::Streaming { target } => {
                let Some(ref location) = target.location else {
                    return Err(RecordingSessionError::NoLocation(id));
                };

                Self::start_livestream(
                    &mut self.mixer,
                    location.to_string(),
                    &format!("Livestream-{id}"),
                )
                .map_err(RecordingSessionError::StartLivestream)
            }
            RecorderStreamKind::Recording { file_name } => {
                Self::start_recording(&mut self.mixer, &self.temp_dir, file_name.as_str())
                    .map_err(RecordingSessionError::StartRecording)
            }
        };

        stream.state = match new_state {
            Ok(()) => StreamStatus::Active,
            Err(error) => StreamStatus::Error {
                reason: error.into(),
            },
        };

        Ok(stream.state.clone())
    }

    async fn stop_stream(
        &mut self,
        id: StreamingTargetId,
    ) -> Result<StreamStatus, RecordingSessionError> {
        log::trace!("stop_stream, id: {id:?}");
        let Some(stream) = self.streaming_targets.get_mut(&id) else {
            return Err(RecordingSessionError::NotFound(id));
        };
        if stream.state != StreamStatus::Active && stream.state != StreamStatus::Paused {
            return Err(RecordingSessionError::NotRunning(id));
        }
        let new_state = match stream.kind {
            RecorderStreamKind::Recording { file_name: _ } => {
                self.mixer.release_sink(&"recording".to_owned()).await;

                self.service_context
                    .upload(
                        &self.room_id,
                        self.temp_dir.path().join(TEMP_RECORDING_NAME).as_path(),
                    )
                    .await
                    .map_err(RecordingSessionError::UploadRecording)
            }
            RecorderStreamKind::Streaming { target: _ } => {
                self.mixer.release_sink(&format!("Livestream-{id}")).await;

                Ok(())
            }
        };

        stream.state = match new_state {
            Ok(()) => StreamStatus::Inactive,
            Err(error) => StreamStatus::Error {
                reason: error.into(),
            },
        };

        Ok(stream.state.clone())
    }

    pub fn start_livestream(mixer: &mut Mixer, location: String, name: &str) -> Result<()> {
        mixer.link_sink(
            name,
            RTMPSink::create(
                name,
                RTMPParameters {
                    location,
                    audio_bitrate: None,
                    audio_rate: None,
                    video_bitrate: None,
                    video_speed_preset: None,
                },
            )
            .context("RTMPSink could not created")?,
        )?;

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
            Event::JoinSuccess {
                participant_id,
                event_title,
                streaming_targets,
            } => {
                self.handle_join_success(streaming_targets, event_title, participant_id)
                    .await?;
            }

            Event::ParticipantJoined(id) => {
                log::debug!("Event::ParticipantJoined");
                self.handle_participant_joined(id).await?;
            }
            Event::ParticipantUpdated(id) => {
                log::debug!("Event::ParticipantUpdated");
                self.handle_participant_updated(id).await?;
            }
            Event::ParticipantLeft(id) => {
                log::debug!("Event::ParticipantLeft");
                self.handle_participant_left(id)?;
            }
            Event::SdpOffer(descriptor, offer) => {
                log::debug!("Event::SdpOffer");
                if let Some(source) = self.mixer.get_source(descriptor) {
                    let answer = source.receive_offer(offer).await?;
                    self.signaling.send_answer(descriptor, answer).await?;

                    // Insert descriptor to configuration set to track
                    self.configurations.insert(descriptor, true);
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
                self.handle_end_of_candidates(descriptor)?;
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
            Event::SpeakerUpdated(speaking_state) => {
                log::debug!("Event::SpeakerUpdated");
                let participant = speaking_state.participant;
                if speaking_state.speaker.is_speaking {
                    self.mixer
                        .set_speaker(participant)
                        .context("unable to set speaker for '{participant}'")?;
                }
            }
            Event::Start(target_ids) => {
                log::debug!("[Start]: {target_ids:#?}");
                self.handle_start_event(target_ids).await?;
            }
            Event::Pause(target_ids) => {
                log::debug!("[Pause]: {target_ids:#?}");
            }
            Event::Stop(target_ids) => {
                log::debug!("[Stop]: {target_ids:#?}");
                self.handle_stop_event(target_ids).await?;
            }
        }

        self.send_configuration().await?;

        Ok(())
    }

    /// Send the configuration if the state changed. For example a participant
    /// updates there media or if a new participant is not shown in the
    /// recording.
    async fn send_configuration(&mut self) -> Result<()> {
        let visibles = self.mixer.get_visibles();

        let configurations_to_add = self
            .mixer
            .get_visibles()
            .into_iter()
            .collect::<HashSet<_>>();
        let configurations_to_remove = self
            .configurations
            .keys()
            .filter(|key| !visibles.contains(key))
            .copied()
            .collect::<HashSet<_>>();

        for descriptor in configurations_to_add {
            let old_configuration = self.configurations.insert(descriptor, true);
            if old_configuration.is_none() || old_configuration == Some(false) {
                self.signaling
                    .send_configuration(descriptor, true)
                    .await
                    .context("unable to send configuration")?;
            }
        }

        for descriptor in configurations_to_remove {
            let old_configuration = self.configurations.insert(descriptor, false);
            if old_configuration.is_none() || old_configuration == Some(true) {
                self.signaling
                    .send_configuration(descriptor, false)
                    .await
                    .context("unable to send configuration")?;
            }
        }

        Ok(())
    }

    async fn handle_join_success(
        &mut self,
        streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamInfo>,
        event_title: String,
        participant_id: ParticipantId,
    ) -> Result<(), anyhow::Error> {
        self.participant_id = Some(participant_id);
        self.streaming_targets = streaming_targets
            .iter()
            .map(|(id, target)| {
                (
                    *id,
                    match target {
                        RecorderStreamInfo::Recording(target) => RecorderStreamStatus {
                            state: target.stream_start_options.status.clone(),
                            kind: RecorderStreamKind::Recording {
                                file_name: TEMP_RECORDING_NAME.to_owned(),
                            },
                        },
                        RecorderStreamInfo::Streaming(target) => RecorderStreamStatus {
                            state: target.stream_start_options.status.clone(),
                            kind: RecorderStreamKind::Streaming {
                                target: target.clone(),
                            },
                        },
                    },
                )
            })
            .collect();

        if self
            .streaming_targets
            .iter()
            .all(|(_, rec_info)| rec_info.state == StreamStatus::Inactive)
        {
            log::debug!("No streams to start requested, recorder is done.");
            self.done = true;
            return Ok(());
        }

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
        for (id, _status) in streaming_targets
            .iter()
            .filter(|(_id, state)| state.is_start_requested())
        {
            let status = match self.start_stream(*id) {
                Ok(status) => status,
                Err(reason) => StreamStatus::Error {
                    reason: reason.into(),
                },
            };

            self.signaling
                .send_stream_update(*id, status)
                .await
                .context("unable to send stream update")?;
        }
        for (id, display_name, media_type, media_state) in available_media_streams {
            log::debug!("JoinSuccess: subscribe stream of {id} {media_type}");
            let descriptor = MediaDescriptor {
                participant_id: id,
                media_type,
            };
            self.subscribe(descriptor, &display_name, media_state)
                .await?;
        }
        self.mixer.set_title(&event_title);
        Ok(())
    }

    async fn handle_participant_joined(&mut self, id: ParticipantId) -> Result<(), anyhow::Error> {
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

        Ok(())
    }

    async fn handle_participant_updated(&mut self, id: ParticipantId) -> Result<(), anyhow::Error> {
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
                    self.subscribe(descriptor, &participant_state.display_name, media_state)
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

        Ok(())
    }

    fn handle_participant_left(&mut self, id: ParticipantId) -> Result<()> {
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
            log::debug!("Last participant left the session. Stop recording.");
            self.done = true;

            return Ok(());
        }

        log::trace!(
            "{} remaining participants : {:?}",
            self.signaling.participants().len(),
            self.signaling.participants().keys()
        );

        Ok(())
    }

    fn handle_end_of_candidates(
        &mut self,
        descriptor: MediaDescriptor,
    ) -> Result<(), anyhow::Error> {
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
        Ok(())
    }

    async fn handle_start_event(
        &mut self,
        target_ids: BTreeSet<StreamingTargetId>,
    ) -> Result<(), anyhow::Error> {
        for id in target_ids {
            let status = match self.start_stream(id) {
                Ok(status) => status,
                Err(reason) => StreamStatus::Error {
                    reason: reason.into(),
                },
            };

            self.signaling
                .send_stream_update(id, status)
                .await
                .context("unable to send stream update")?;
        }

        Ok(())
    }

    async fn handle_stop_event(
        &mut self,
        target_ids: BTreeSet<StreamingTargetId>,
    ) -> Result<(), anyhow::Error> {
        for id in target_ids {
            let status = match self.stop_stream(id).await {
                Ok(status) => status,
                Err(reason) => StreamStatus::Error {
                    reason: reason.into(),
                },
            };

            self.signaling
                .send_stream_update(id, status)
                .await
                .context("unable to send stream update")?;
        }

        if !self
            .streaming_targets
            .iter()
            .any(|(_id, status)| status.stream_running())
        {
            // last stream has been stopped, the media pipeline can be shut down.
            self.done = true;
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
