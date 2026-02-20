// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::{
    pin::Pin,
    task::{ready, Context, Poll},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    io, mem,
    sync::Arc,
};

use anyhow::{Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{
    livekit::prelude::DisconnectReason, EncoderType, Mixer, MixerParameters, ParticipantIdentity,
    RTMPParameters, RTMPSink, SystemSink, WebMParameters, WebMSink,
};
use futures::Stream;
use log::error;
use opentalk_client::{
    types::{
        common::{streaming::StreamingTargetId, time::Timestamp},
        signaling::{
            livekit::Credentials,
            recording::{StreamErrorReason, StreamStatus},
            recording_service::state::{RecorderStreamInfo, StreamingTarget},
            ParticipantId,
        },
    },
    Event, OpenTalkClient, OpenTalkEvent, OpenTalkRecordingServiceEvent, Participant, Room,
};
use opentalk_recorder_web_api::v1::RecordingTarget;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, ReadBuf},
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use tokio_stream::wrappers::BroadcastStream;

#[derive(Debug, Clone, thiserror::Error)]
#[error("reached chunk limit")]
#[allow(dead_code)]
pub struct ChunkUploadLimitReached;

#[derive(Debug, Clone)]
pub struct UploadLimitReached(StreamingTargetId);

impl UploadLimitReached {
    pub fn id(&self) -> &StreamingTargetId {
        &self.0
    }
}

// TODO: Replace with version from opentalk-types
#[derive(Clone)]
pub(crate) struct FileExtension(String);

impl FileExtension {
    #[must_use]
    pub(crate) fn webm() -> Self {
        Self("webm".to_string())
    }

    #[must_use]
    pub(crate) fn str(&self) -> &str {
        &self.0
    }
}

use crate::settings::Settings;

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

pub(crate) struct Recorder {
    pub(crate) settings: Arc<Settings>,
    pub(crate) client: Arc<OpenTalkClient>,
    pub(crate) shutdown: broadcast::Receiver<()>,
}

impl Clone for Recorder {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            client: self.client.clone(),
            shutdown: self.shutdown.resubscribe(),
        }
    }
}

impl Recorder {
    /// This constructor is used by the integration tests to mock data.
    pub(crate) fn new(
        settings: Arc<Settings>,
        client: OpenTalkClient,
        shutdown: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            settings,
            client: Arc::new(client),
            shutdown,
        }
    }

    pub(crate) async fn spawn_session(
        &self,
        recording_target: RecordingTarget,
    ) -> Result<JoinHandle<Result<()>>> {
        let context = Arc::new(self.clone());
        log::debug!("Start Recording session {recording_target:?}");
        let mut session = Box::pin(RecordingSession::create(context, recording_target))
            .await
            .context("recording session failed to start")?;

        let recording_task = tokio::spawn(async move {
            if let Err(ref recording_err) = Box::pin(session.run()).await {
                error!("recording session failed but trying upload anyway:\n{recording_err:?}",);
            }

            Ok(())
        });

        Ok(recording_task)
    }
}

#[derive(Debug)]
pub(crate) struct InitialData {
    pub streaming_targets: BTreeMap<StreamingTargetId, RecorderStreamStatus>,

    pub livekit_data: Credentials,
}

impl Default for InitialData {
    fn default() -> Self {
        Self {
            streaming_targets: BTreeMap::new(),
            livekit_data: Credentials {
                room: String::new(),
                token: String::new(),
                public_url: String::new(),
                service_url: None,
            },
        }
    }
}

pub(crate) struct RecordingSession {
    service_context: Arc<Recorder>,
    room_state: opentalk_client::Room,

    room_id: String,
    participant_id: Option<ParticipantId>,

    mixer: Option<Mixer>,

    initial_data: InitialData,

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

    #[error("Mixer does not exist")]
    MixerError,
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
            RecordingSessionError::MixerError => "internal_error".to_owned(),
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
        room_state: Room,
        room_id: String,
        mixer: Option<Mixer>,
        initial_data: InitialData,
        done: bool,
    ) -> Self {
        Self {
            service_context,
            room_state,
            room_id,
            participant_id: None,
            mixer,
            initial_data,
            done,
            join_handles: Arc::default(),
        }
    }

    pub(crate) async fn create(
        service_context: Arc<Recorder>,
        recording_target: RecordingTarget,
    ) -> Result<RecordingSession> {
        let room_state = Box::pin(service_context.client.connect_recorder(
            recording_target.room,
            recording_target.breakout,
            true,
        ))
        .await?
        .into();

        let livekit_room = if let Some(breakout) = recording_target.breakout {
            format!("{}:{breakout}", recording_target.room)
        } else {
            recording_target.room.to_string()
        };

        Ok(Self {
            service_context,
            room_state,
            room_id: livekit_room,
            participant_id: None,
            mixer: None,
            initial_data: InitialData::default(),
            done: false,
            join_handles: Arc::default(),
        })
    }

    pub(crate) async fn run(&mut self) -> Result<()> {
        async fn mixer_run(mixer: Option<&mut Mixer>) -> DisconnectReason {
            let Some(mixer) = mixer else {
                std::future::pending().await
            };

            mixer.run().await
        }

        let mut shutdown_rx = self.service_context.shutdown.resubscribe();
        let (chunk_limit_reached_tx, mut chunk_limit_reached_rx) =
            broadcast::channel::<UploadLimitReached>(1);

        Box::pin(self.configure_mixer(Some(chunk_limit_reached_tx.clone()))).await?;

        while !self.done {
            tokio::select! {
                msg = self.room_state.recv() => {
                    match msg {
                        Err(err) => {
                            log::debug!("Unexpected websocket message. {err}");
                        },
                        Ok(event) => match event {
                            Event::OpenTalk(open_talk_event) => Box::pin(self.handle_signaling_event(open_talk_event, chunk_limit_reached_tx.clone())).await?,
                            Event::LiveKit(_) => {},
                        }
                    }
                }
                disconnect_reason = mixer_run(self.mixer.as_mut()) => {
                    log::error!("Disconnected from livekit: {disconnect_reason:?}");
                    break;
                }
                chunk_limit_stream = chunk_limit_reached_rx.recv() => {
                    let id = *chunk_limit_stream?.id();
                    self.handle_stop_stream(BTreeSet::from([id])).await?;
                }
                _ = shutdown_rx.recv() => {
                    self.done = true;
                    break;
                }
            }
        }

        // The streaming targets are per session
        // therefore making sure we're in the right context isn't necessary here.
        log::debug!("Recorder is done, attempting to upload remaining streams...");
        let cloned_stream = self.initial_data.streaming_targets.clone();
        for (stream_target_id, _) in cloned_stream
            .iter()
            .filter(|(_, status)| RecorderStreamStatus::stream_running(status))
        {
            self.stop_stream(*stream_target_id).await?;
        }

        Ok(())
    }

    pub(crate) async fn start_recording(
        &mut self,
        id: StreamingTargetId,
        encoder_type: EncoderType,
        sender: broadcast::Sender<UploadLimitReached>,
    ) -> Result<()> {
        let webm_sink = WebMSink::create(&WebMParameters {
            encoder_type,
            chunk_size: Some(self.service_context.settings.controller.upload_chunk_size as u64),
        })
        .context("WebM-Sink could not created")?;

        // probably actually use a channel to signal when limit reached, would probably make much more sense
        // than to attempt to circumvent the move
        let handle = tokio::spawn({
            let service_context = self.service_context.clone();
            let room_id = self.room_id.clone();
            let receiver = webm_sink.subscribe();

            async move {
                let timestamp = Timestamp::now();
                let uri = format!(
                    "{}/services/recording/upload?room_id={room_id}&file_extension={}&timestamp={}",
                    service_context
                        .settings
                        .controller
                        .v1_api_base_url()
                        .replace("https", "wss")
                        .replace("http", "ws"),
                    FileExtension::webm().str(),
                    urlencoding::encode(&timestamp.to_string()),
                );
                let stream = BroadcastStream::from(receiver);

                match service_context
                    .client
                    .upload_render(uri, stream, || {
                        if let Err(e) = sender.send(UploadLimitReached(id)) {
                            log::warn!("Could not send Upload limit reached of {id} because: {e}");
                        }
                    })
                    .await
                {
                    Ok(()) => {}
                    Err(e) => return Err(anyhow::anyhow!(format!("Error: {e}"))),
                }

                Ok(())
            }
        });

        self.join_handles.lock().await.push(handle);

        self.mixer
            .as_mut()
            .context("Mixer does not exist")?
            .link_gstreamer_sink("recording", webm_sink)
            .await
            .context("unable to link recording sink to compositor")?;

        Ok(())
    }

    async fn start_stream(
        &mut self,
        id: StreamingTargetId,
        chunk_limit_sender: Option<broadcast::Sender<UploadLimitReached>>,
    ) -> Result<StreamStatus, RecordingSessionError> {
        log::trace!("start_stream, id: {id:?}");
        let Some(mut stream) = self.initial_data.streaming_targets.get_mut(&id).cloned() else {
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
                let Some(chunk_limit_sender) = chunk_limit_sender else {
                    return Err(anyhow::anyhow!("Chunk Limit sender doesn't exist"))
                        .map_err(RecordingSessionError::StartRecording);
                };
                let result = self
                    .start_recording(
                        id,
                        self.service_context.settings.encoder_type(),
                        chunk_limit_sender.clone(),
                    )
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

        self.initial_data
            .streaming_targets
            .insert(id, stream.clone());

        Ok(stream.state.clone())
    }

    async fn stop_stream(
        &mut self,
        id: StreamingTargetId,
    ) -> Result<StreamStatus, RecordingSessionError> {
        log::trace!("stop_stream, id: {id:?}");
        let Some(stream) = self.initial_data.streaming_targets.get_mut(&id) else {
            return Err(RecordingSessionError::NotFound(id));
        };
        if stream.state != StreamStatus::Active && stream.state != StreamStatus::Paused {
            return Err(RecordingSessionError::NotRunning(id));
        }
        let new_state: Result<(), StreamErrorReason> = match stream.kind {
            RecorderStreamKind::Recording => {
                self.mixer
                    .as_mut()
                    .ok_or(RecordingSessionError::MixerError)?
                    .release_sink(&"recording".to_owned())
                    .await;

                Ok(())
            }
            RecorderStreamKind::Streaming { target: _ } => {
                self.mixer
                    .as_mut()
                    .ok_or(RecordingSessionError::MixerError)?
                    .release_sink(&format!("Livestream-{id}"))
                    .await;

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
            .as_mut()
            .context("mixer does not exist")?
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

    async fn handle_signaling_event(
        &mut self,
        event: OpenTalkEvent,
        chunk_limit_sender: broadcast::Sender<UploadLimitReached>,
    ) -> Result<()> {
        log::trace!("Received: {event:?} event message");
        match event {
            OpenTalkEvent::ParticipantJoined(participant) => {
                self.handle_participant_joined(&participant)?;
            }
            OpenTalkEvent::ParticipantUpdated {
                previous: _,
                updated,
            } => self.handle_participant_updated(&updated)?,
            OpenTalkEvent::ParticipantLeft(left) => self.handle_participant_left(&left)?,
            OpenTalkEvent::MovedToWaitingRoom
            | OpenTalkEvent::WaitingRoomAccepted
            | OpenTalkEvent::LiveKit(_)
            | OpenTalkEvent::Recording(_)
            | OpenTalkEvent::Chat(_)
            | OpenTalkEvent::Disconnected(_)
            | OpenTalkEvent::TranscriptionService(_) => {
                log::trace!("Got event: {event:?}");
                return Ok(());
            }
            OpenTalkEvent::RecordingService(open_talk_recording_service_event) => {
                match open_talk_recording_service_event {
                    OpenTalkRecordingServiceEvent::StartStreams { target_ids } => {
                        log::debug!("[Start]: {target_ids:?}");
                        self.handle_start_stream(target_ids, chunk_limit_sender)
                            .await?;
                    }
                    OpenTalkRecordingServiceEvent::PauseStreams { target_ids } => {
                        log::debug!("[Pause]: {target_ids:?}");
                    }
                    OpenTalkRecordingServiceEvent::StopStreams { target_ids } => {
                        log::debug!("[Stop]: {target_ids:?}");
                        self.handle_stop_stream(target_ids).await?;
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn configure_mixer(
        &mut self,
        chunk_limit_sender: Option<broadcast::Sender<UploadLimitReached>>,
    ) -> Result<()> {
        self.participant_id = Some(self.room_state.participant_id());
        self.initial_data.streaming_targets = self
            .room_state
            .recording_service_api()
            .context("no streams available")?
            .all_streams()
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

        if let Some(livekit_credentials) = self.room_state.livekit_credentials() {
            self.initial_data.livekit_data = livekit_credentials.clone();
        }

        let recorder_settings = self
            .service_context
            .settings
            .recorder
            .clone()
            .unwrap_or_default();

        let mixer_params = MixerParameters {
            target_fps: 30,
            auto_subscribe: false,
            clock_format: recorder_settings.clock_format,
            livekit_url: self
                .initial_data
                .livekit_data
                .service_url
                .as_mut()
                .context("service url does not exist")?
                .clone(),
            livekit_token: self.initial_data.livekit_data.token.clone(),
        };

        self.mixer = Some(
            Box::pin(Mixer::new(mixer_params))
                .await
                .context("Mixer could not be created")?,
        );

        if recorder_settings.display {
            let system_sink = SystemSink::create().context("DisplaySink could not created")?;
            self.mixer
                .as_mut()
                .context("mixer does not exist")?
                .link_gstreamer_sink("Display", system_sink)
                .await?;
        }

        if self
            .initial_data
            .streaming_targets
            .iter()
            .all(|(_, rec_info)| rec_info.state == StreamStatus::Inactive)
        {
            log::debug!("No streams to start requested, recorder is done.");
            self.done = true;
            return Ok(());
        }

        let all_streams = {
            let recording_service_api = self.room_state.recording_service_api().unwrap();
            recording_service_api.all_streams().clone()
        };

        for (id, _status) in all_streams
            .into_iter()
            .filter(|(_id, state)| state.is_start_requested())
        {
            let status = match self.start_stream(id, chunk_limit_sender.clone()).await {
                Ok(status) => status,
                Err(reason) => StreamStatus::Error {
                    reason: reason.into(),
                },
            };

            self.room_state
                .recording_service_api()
                .unwrap()
                .send_stream_update(id, status)
                .await?;
        }

        if let Some(event_title) = self.room_state.event_title() {
            self.mixer
                .as_mut()
                .context("mixer does not exist")?
                .set_event_title(event_title);
        }

        for participant in self.room_state.participants().values() {
            if participant.consents_recording {
                self.mixer
                    .as_mut()
                    .context("mixer does not exist")?
                    .add_participant(
                        &ParticipantIdentity::from(participant.id.to_string()),
                        participant.display_name.to_string(),
                    );
            }
        }

        Ok(())
    }

    fn handle_participant_joined(&mut self, participant: &Participant) -> Result<()> {
        if participant.consents_recording {
            self.mixer
                .as_mut()
                .context("mixer does not exist")?
                .add_participant(
                    &ParticipantIdentity::from(participant.id.to_string()),
                    participant.display_name.to_string(),
                );
        }

        Ok(())
    }

    fn handle_participant_updated(&mut self, updated: &Participant) -> Result<()> {
        if updated.consents_recording {
            self.mixer
                .as_mut()
                .context("mixer does not exist")?
                .add_participant(
                    &ParticipantIdentity::from(updated.id.to_string()),
                    updated.display_name.to_string(),
                );
        }

        Ok(())
    }

    fn handle_participant_left(&mut self, left: &ParticipantId) -> Result<()> {
        let active_participants = self.room_state.active_participants();
        if active_participants.is_empty() {
            log::debug!("Last participant left the session. Stop recording.");
            self.done = true;

            return Ok(());
        }

        log::trace!(
            "{} remaining participants : {:?}",
            active_participants.len(),
            active_participants.keys()
        );

        self.mixer
            .as_mut()
            .context("mixer does not exist")?
            .remove_participant(&ParticipantIdentity::from(left.to_string()));

        Ok(())
    }

    pub(crate) async fn handle_start_stream(
        &mut self,
        target_ids: BTreeSet<StreamingTargetId>,
        chunk_limit_sender: broadcast::Sender<UploadLimitReached>,
    ) -> Result<()> {
        for id in target_ids {
            let status = match self
                .start_stream(id, Some(chunk_limit_sender.clone()))
                .await
            {
                Ok(status) => status,
                Err(reason) => StreamStatus::Error {
                    reason: reason.into(),
                },
            };

            self.room_state
                .recording_service_api()
                .unwrap()
                .send_stream_update(id, status)
                .await?;
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

            self.room_state
                .recording_service_api()
                .unwrap()
                .send_stream_update(id, status)
                .await?;
        }

        if !self
            .initial_data
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
