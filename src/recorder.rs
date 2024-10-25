// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Debug,
    io, mem,
    sync::Arc,
};

use anyhow::{Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{
    EncoderType, Mixer, MixerParameters, ParticipantIdentity, RTMPParameters, RTMPSink, SystemSink,
    WebMParameters, WebMSink,
};
use futures::Stream;
use log::error;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, ReadBuf},
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use types::signaling::{
    recording::{
        state::{RecorderStreamInfo, StreamingTarget},
        StreamErrorReason, StreamStatus,
    },
    recording_service::command::RecordingServiceCommand,
};
use types_common::streaming::StreamingTargetId;
use types_control::event::{ControlEvent, Left};
use types_signaling::{Participant, ParticipantId};

use crate::{
    http::{FileExtension, HttpClient},
    rmq::InitializeRecording,
    settings::Settings,
    signaling::{self, incoming, ParticipantState, Signaling},
};

#[derive(Clone, Debug)]
pub(crate) enum RecorderStreamKind {
    Recording,
    Streaming { target: StreamingTarget },
}

#[derive(Clone, Debug)]
pub(crate) struct RecorderStreamStatus {
    pub(crate) state: StreamStatus,
    pub(crate) kind: RecorderStreamKind,
}

impl RecorderStreamStatus {
    #[must_use]
    pub(crate) fn stream_running(&self) -> bool {
        self.state == StreamStatus::Active
            || self.state == StreamStatus::Paused
            || self.state == StreamStatus::Starting
    }
}

#[derive(Debug)]
pub(crate) struct Recorder {
    pub(crate) settings: Arc<Settings>,
    pub(crate) http_client: Arc<HttpClient>,
    pub(crate) shutdown: broadcast::Receiver<()>,
}

impl Clone for Recorder {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            http_client: self.http_client.clone(),
            shutdown: self.shutdown.resubscribe(),
        }
    }
}

impl Recorder {
    /// This constructor is used by the integration tests to mock data.
    pub(crate) fn new(
        settings: Settings,
        http_client: HttpClient,
        shutdown: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            settings: Arc::new(settings),
            http_client: Arc::new(http_client),
            shutdown,
        }
    }

    pub(crate) async fn spawn_session(
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
}

#[derive(Debug)]
pub(crate) struct RecordingSession {
    service_context: Arc<Recorder>,

    signaling: Signaling,

    /// List of all other participants in the conference
    participants: HashMap<ParticipantId, ParticipantState>,

    room_id: String,
    participant_id: Option<ParticipantId>,

    mixer: Mixer,

    streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamStatus>,

    done: bool,

    join_handles: Arc<Mutex<Vec<JoinHandle<Result<()>>>>>,
}

#[derive(Debug, Error)]
pub(crate) enum RecordingSessionError {
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

    #[error("Start recording failed, reason: {0}")]
    StartRecording(anyhow::Error),
}

impl From<RecordingSessionError> for StreamErrorReason {
    fn from(value: RecordingSessionError) -> Self {
        let code = match value {
            RecordingSessionError::AlreadyRunning(_) => "already_running".to_owned(),
            RecordingSessionError::NotFound(_) => "not_found".to_owned(),
            RecordingSessionError::NotRunning(_) => "not_running".to_owned(),
            RecordingSessionError::NoLocation(_) => "no_location".to_owned(),

            RecordingSessionError::StartLivestream(_) => "start_livestream".to_owned(),

            RecordingSessionError::StartRecording(_) => "start_recording".to_owned(),
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
    pub(crate) fn new(
        service_context: Arc<Recorder>,
        signaling: Signaling,
        participants: HashMap<ParticipantId, ParticipantState>,
        room_id: String,
        mixer: Mixer,
        streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamStatus>,
        done: bool,
    ) -> Self {
        Self {
            service_context,
            signaling,
            participants,
            room_id,
            participant_id: None,
            mixer,
            streaming_targets,
            done,
            join_handles: Arc::default(),
        }
    }

    pub(crate) async fn create(
        service_context: Arc<Recorder>,
        command: InitializeRecording,
    ) -> Result<RecordingSession> {
        let signaling = Signaling::connect(
            service_context.http_client.as_ref(),
            &service_context.settings.controller,
            &command.room,
            &command.breakout,
        )
        .await
        .context("Failed to connect to signaling")?;

        let recorder_settings = service_context
            .settings
            .recorder
            .clone()
            .unwrap_or_default();

        let livekit_room = if let Some(breakout) = command.breakout {
            format!("{}:{breakout}", command.room)
        } else {
            command.room.clone()
        };

        let mixer_parameters = MixerParameters {
            auto_subscribe: false,
            video_support: true,
            clock_format: service_context
                .settings
                .recorder
                .clone()
                .map(|recorder_settings| recorder_settings.clock_format)
                .unwrap_or_default(),
            livekit_url: service_context.settings.livekit.url.clone(),
            livekit_api_key: service_context.settings.livekit.api_key.clone(),
            livekit_api_secret: service_context.settings.livekit.api_secret.clone(),
            livekit_room,
        };

        let mut mixer = Mixer::new(mixer_parameters).await?;

        if recorder_settings.display {
            let system_sink = SystemSink::create(true).context("DisplaySink could not created")?;
            mixer.link_gstreamer_sink("Display", system_sink).await?;
        }

        Ok(Self {
            service_context,
            signaling,
            participants: HashMap::new(),
            room_id: command.room,
            participant_id: None,
            mixer,
            streaming_targets: BTreeMap::new(),
            done: false,
            join_handles: Arc::default(),
        })
    }

    pub(crate) async fn run(&mut self) -> Result<()> {
        let mut shutdown_rx = self.service_context.shutdown.resubscribe();

        while !self.done {
            tokio::select! {
                msg = self.signaling.recv_new_signal() => {
                    match msg {
                        Err(err) => {
                            log::debug!("Unexpected websocket message. {err}");
                            continue;
                        },
                        Ok(Some(msg)) => {
                            self.handle_signaling_event(msg).await?;
                        }
                        Ok(None) => {
                            log::trace!("Received None message");
                            continue;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    self.done = true;
                    break;
                }
                result = self.mixer.run() => {
                    if let Err(err) = result {
                        log::error!("Running the mixer caused an error: {err:?}");
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

    pub(crate) async fn start_recording(&mut self, encoder_type: EncoderType) -> Result<()> {
        let webm_sink = WebMSink::create(&WebMParameters { encoder_type })
            .context("WebM-Sink could not created")?;

        let handle = tokio::spawn({
            let service_context = self.service_context.clone();
            let room_id = self.room_id.clone();
            let receiver = webm_sink.subscribe();

            async move {
                service_context
                    .http_client
                    .upload_render(
                        &service_context.settings.controller,
                        &room_id,
                        FileExtension::webm(),
                        receiver,
                    )
                    .await?;

                Ok(())
            }
        });
        self.join_handles.lock().await.push(handle);

        self.mixer
            .link_gstreamer_sink("recording", webm_sink)
            .await
            .context("unable to link recording sink to compositor")?;

        Ok(())
    }

    async fn start_stream(
        &mut self,
        id: StreamingTargetId,
    ) -> Result<StreamStatus, RecordingSessionError> {
        log::trace!("start_stream, id: {id:?}");
        let Some(mut stream) = self.streaming_targets.get_mut(&id).cloned() else {
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

                let result = self
                    .start_livestream(
                        location.to_string(),
                        &format!("Livestream-{id}"),
                        self.service_context.settings.encoder_type(),
                    )
                    .await;

                if let Err(ref e) = result {
                    log::error!("failed to start live stream: {e}");
                }

                result.map_err(RecordingSessionError::StartLivestream)
            }
            RecorderStreamKind::Recording => {
                let result = self
                    .start_recording(self.service_context.settings.encoder_type())
                    .await;

                if let Err(ref e) = result {
                    log::error!("failed to start recording: {e}");
                }

                result.map_err(RecordingSessionError::StartRecording)
            }
        };

        stream.state = match new_state {
            Ok(()) => StreamStatus::Active,
            Err(error) => StreamStatus::Error {
                reason: error.into(),
            },
        };

        self.streaming_targets.insert(id, stream.clone());

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
        let new_state: Result<(), StreamErrorReason> = match stream.kind {
            RecorderStreamKind::Recording => {
                self.mixer.release_sink(&"recording".to_owned()).await;

                Ok(())
            }
            RecorderStreamKind::Streaming { target: _ } => {
                self.mixer.release_sink(&format!("Livestream-{id}")).await;

                Ok(())
            }
        };

        stream.state = match new_state {
            Ok(()) => StreamStatus::Inactive,
            Err(error) => StreamStatus::Error { reason: error },
        };

        Ok(stream.state.clone())
    }

    pub async fn start_livestream(
        &mut self,
        location: String,
        name: &str,
        encoder_type: EncoderType,
    ) -> Result<()> {
        self.mixer
            .link_gstreamer_sink(
                name,
                RTMPSink::create(RTMPParameters {
                    location,
                    audio_bitrate: None,
                    audio_rate: None,
                    video_bitrate: None,
                    video_speed_preset: None,
                    encoder_type,
                })
                .context("RTMPSink could not created")?,
            )
            .await
            .context("unable to link sink to talk")?;

        Ok(())
    }

    async fn handle_signaling_event(&mut self, msg: incoming::Message) -> Result<()> {
        match msg {
            incoming::Message::Control(msg) => match msg {
                ControlEvent::JoinSuccess(state) => {
                    let Ok(participants) = signaling::process_participants(&state) else {
                        return Ok(());
                    };
                    self.participants = participants;

                    let Ok(streams) = signaling::handle_join_success(&state) else {
                        log::info!("No Streaming targets in ControlEvent::JoinSuccess found.");
                        return Ok(());
                    };

                    let Some(event) = state.event_info else {
                        log::info!("No Event info found in ControlEvent::JoinSuccess.");
                        return Ok(());
                    };

                    self.handle_join_success(
                        streams,
                        event.title.to_string(),
                        state.id,
                        state.participants,
                    )
                    .await?;
                }
                ControlEvent::Joined(participant) => {
                    signaling::handle_joined(&participant, &mut self.participants)?;
                    self.handle_participant_joined(participant.id).await?;
                }
                ControlEvent::Update(participant) => {
                    signaling::handle_update(&participant, &mut self.participants);
                    self.handle_participant_updated(participant.id).await?;
                }
                ControlEvent::Left(Left {
                    id: assoc_participant,
                    ..
                }) => {
                    signaling::handle_left(&assoc_participant, &mut self.participants);
                    self.handle_participant_left(assoc_participant.id).await?;
                }
                ref other => {
                    log::error!("Event {other:#?} not implemented for recorder.");
                    return Ok(());
                }
            },
            incoming::Message::RecordingService(msg) => match msg {
                RecordingServiceCommand::StartStreams { target_ids } => {
                    log::debug!("[Start]: {target_ids:#?}");
                    self.handle_start_stream(target_ids).await?;
                }
                RecordingServiceCommand::PauseStreams { target_ids } => {
                    log::debug!("[Pause]: {target_ids:#?}");
                }
                RecordingServiceCommand::StopStreams { target_ids } => {
                    log::debug!("[Stop]: {target_ids:#?}");
                    self.handle_stop_stream(target_ids).await?;
                }
            },
        }
        Ok(())
    }

    pub(crate) async fn handle_join_success(
        &mut self,
        streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamInfo>,
        event_title: String,
        participant_id: ParticipantId,
        participants: Vec<Participant>,
    ) -> Result<()> {
        self.participant_id = Some(participant_id);
        self.streaming_targets = streaming_targets
            .iter()
            .map(|(id, target)| {
                (
                    *id,
                    match target {
                        RecorderStreamInfo::Recording(target) => RecorderStreamStatus {
                            state: target.stream_start_options.status.clone(),
                            kind: RecorderStreamKind::Recording,
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

        for (id, _status) in streaming_targets
            .iter()
            .filter(|(_id, state)| state.is_start_requested())
        {
            let status = match self.start_stream(*id).await {
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

        self.mixer.set_event_title(event_title).await;

        for participant in participants {
            let participant_state = self
                .participants
                .get(&participant.id)
                .context("participant not found")?
                .clone();

            if participant_state.consents {
                self.mixer
                    .add_participant(
                        ParticipantIdentity::from(participant.id.to_string()),
                        participant_state.display_name,
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn handle_participant_joined(&mut self, id: ParticipantId) -> Result<()> {
        let participant_state = self
            .participants
            .get(&id)
            .context("participant not found")?
            .clone();

        if participant_state.consents {
            self.mixer
                .add_participant(
                    ParticipantIdentity::from(id.to_string()),
                    participant_state.display_name,
                )
                .await;
        }

        Ok(())
    }

    async fn handle_participant_updated(&mut self, id: ParticipantId) -> Result<()> {
        let participant_state = self
            .participants
            .get(&id)
            .context("participant not found")?
            .clone();

        if participant_state.consents {
            self.mixer
                .add_participant(
                    ParticipantIdentity::from(id.to_string()),
                    participant_state.display_name,
                )
                .await;
        }

        Ok(())
    }

    async fn handle_participant_left(&mut self, id: ParticipantId) -> Result<()> {
        if self.participants.is_empty() {
            log::debug!("Last participant left the session. Stop recording.");
            self.done = true;

            return Ok(());
        }

        log::trace!(
            "{} remaining participants : {:?}",
            self.participants.len(),
            self.participants.keys()
        );

        self.mixer
            .remove_participant(&ParticipantIdentity::from(id.to_string()))
            .await;

        Ok(())
    }

    pub(crate) async fn handle_start_stream(
        &mut self,
        target_ids: BTreeSet<StreamingTargetId>,
    ) -> Result<()> {
        for id in target_ids {
            let status = match self.start_stream(id).await {
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

    async fn handle_stop_stream(&mut self, target_ids: BTreeSet<StreamingTargetId>) -> Result<()> {
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
}

impl Drop for RecordingSession {
    fn drop(&mut self) {
        log::debug!("Drop RecordingSession");

        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                let join_handles = mem::take(&mut self.join_handles);
                let mut join_handles = join_handles.lock().await;
                for join_handle in join_handles.iter_mut() {
                    if let Err(err) = join_handle.await {
                        log::error!("JoinHandle received an error: {err:?}");
                    }
                }
            });
        });
        log::debug!("Drop RecordingSession is finished");
    }
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
