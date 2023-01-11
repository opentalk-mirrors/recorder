use crate::http::HttpClient;
use crate::rmq::StartRecording;
use crate::settings::Settings;
use crate::signaling::{media_types, Event, Signaling};
use crate::signaling::{ParticipantId, TrickleCandidate};
use anyhow::Result;
use bytes::Bytes;
use compositor::{StreamId, WebRtcSourceParams};
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use futures::Stream;
use log::error;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::task::{spawn_blocking, JoinHandle};

// TODO; make this configurable
const MAX_VISIBLES: usize = 6;

type Talk = compositor::Talk<compositor::WebRtcSource, ParticipantId>;
type Layout = compositor::Grid;

#[derive(Clone)]
pub struct Recorder {
    pub settings: Arc<Settings>,
    pub http_client: Arc<HttpClient>,
}

impl Recorder {
    pub async fn create(settings: Settings) -> Result<Self> {
        let settings = Arc::new(settings);
        let http_client = Arc::new(HttpClient::discover(&settings.auth).await?);
        Ok(Self {
            settings,
            http_client,
        })
    }

    pub fn spawn_session(&self, command: StartRecording) -> Result<JoinHandle<()>> {
        let context = self.clone();
        log::debug!("Start Recording session {command:?}");
        let recording_task = tokio::spawn(async move {
            let session = RecordingSession::create(&context, command)
                .await
                .expect("Failed to start signaling session");

            if let Err(e) = session.run().await {
                log::error!("Recording session failed: {:?}", e);
            }
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
pub struct RecordingSession<'a> {
    service_context: &'a Recorder,

    signaling: Signaling,

    room_id: String,
    temp_dir: TempDir,

    talk: Talk,

    candidate_receiver: mpsc::Receiver<(ParticipantId, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(ParticipantId, u32, Option<String>)>,

    done: bool,
}

impl<'a> RecordingSession<'a> {
    pub async fn create(
        service_context: &'a Recorder,
        command: StartRecording,
    ) -> Result<RecordingSession<'a>> {
        let mut signaling = Signaling::connect(
            service_context.http_client.as_ref(),
            &service_context.settings.controller,
            &command.room,
        )
        .await?;

        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("out.mp4");

        let (candidate_sender, candidate_receiver) = mpsc::channel(12);

        let mut talk = Talk::new(
            compositor::Size::FHD,
            if let Some(recorder) = &service_context.settings.recorder {
                match recorder.sink.to_lowercase().as_str() {
                    "display" => Box::<compositor::DisplaySinkBuilder>::default(),
                    "matroska" => {
                        if let Some(params) = &service_context.settings.matroska {
                            Box::new(compositor::MatroskaSinkBuilder::new(params.clone()))
                        } else {
                            Box::new(compositor::MatroskaSinkBuilder::new(Default::default()))
                        }
                    }
                    _ => Box::new(compositor::Mp4SinkBuilder::new(compositor::Mp4SinkParams {
                        file_path: file_path
                            .to_str()
                            .expect("failed to convert MP4 file path into string")
                            .into(),
                    })),
                }
            } else {
                todo!()
            },
            Some(MAX_VISIBLES),
        )?;

        // find all participants that publish some stream
        for media_type in media_types() {
            let publishing_participants = signaling
                .participants()
                .iter()
                .filter_map(|(id, state)| {
                    state.publishes(&media_type).is_some().then(|| {
                        (
                            *id,
                            state.display_name.clone(),
                            state.publishes(&media_type).unwrap(),
                        )
                    })
                })
                .collect::<Vec<_>>();

            // Subscribe to above collected participants
            for (id, display_name, initial) in publishing_participants {
                talk.add_stream(
                    StreamId::new(id, media_type),
                    &display_name,
                    participant_params(id, candidate_sender.clone()),
                    initial.into(),
                )?;
                signaling.start_subscribe(id, media_type).await?;
            }
        }
        talk.layout::<Layout>()?;

        Ok(Self {
            service_context,
            signaling,
            room_id: command.room,
            temp_dir,
            talk,
            candidate_receiver,
            candidate_sender,
            done: false,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        while !self.done {
            tokio::select! {
                event = self.signaling.run() => {
                    log::trace!("signaling_event {:?}", event);
                    self.handle_signaling_event(event?).await?;
                }
                candidate = self.candidate_receiver.recv() => {
                    self.handle_candidate(candidate.expect("unreachable")).await?;
                }
            }
        }

        self.upload().await?;

        Ok(())
    }

    async fn handle_signaling_event(&mut self, event: Event) -> Result<()> {
        log::debug!("handle_signaling_event");
        match event {
            Event::ParticipantJoined(id) => {
                log::debug!("Event::ParticipantJoined");
                let state = &self.signaling.participants()[&id].clone();
                for media_type in media_types() {
                    if let Some(media_state) = state.publishes(&media_type) {
                        log::debug!("Join: subscribe stream of {id} {media_type}");
                        self.talk.add_stream(
                            StreamId::new(id, media_type),
                            &state.display_name,
                            participant_params(id, self.candidate_sender.clone()),
                            media_state.into(),
                        )?;
                        self.talk.layout::<Layout>()?;
                        self.signaling.start_subscribe(id, media_type).await?;
                    }
                }
            }
            Event::ParticipantUpdated(id) => {
                log::debug!("Event::ParticipantUpdated");
                let state = self.signaling.participants()[&id].clone();
                for media_type in media_types() {
                    let is_subscribed = self.talk.contains_stream(&StreamId::new(id, media_type));
                    let media_state = state.publishes(&media_type);

                    if !is_subscribed {
                        if let Some(media_state) = media_state {
                            log::debug!("Update: subscribe stream of {id} {media_type}");
                            self.talk.add_stream(
                                StreamId::new(id, media_type),
                                &state.display_name,
                                participant_params(id, self.candidate_sender.clone()),
                                media_state.into(),
                            )?;
                            self.signaling.start_subscribe(id, media_type).await?;
                            if state.consents {
                                self.talk.show(&StreamId::new(id, media_type))?;
                            }
                        }
                    } else if media_state.is_none() {
                        log::debug!("Update: unsubscribe stream of {id} {media_type}");
                        self.talk.remove_stream(StreamId::new(id, media_type))?;
                    } else if let Some(media_state) = media_state {
                        log::debug!(
                            "Update: update status of stream of {id} {media_type} to {media_state}"
                        );
                        self.talk
                            .set_status(&StreamId::new(id, media_type), media_state.into())?;
                    } else {
                        log::trace!(
                            "ignore update for {id}: media_state ({media_state:?}) == is_subscribed ({is_subscribed})"
                        );
                        return Ok(());
                    }
                    self.talk.layout::<Layout>()?;
                }

                return Ok(());
            }
            Event::ParticipantLeft(id) => {
                log::debug!("Event::ParticipantLeft");
                for media_type in media_types() {
                    if self.talk.contains_stream(&StreamId::new(id, media_type)) {
                        self.talk.remove_stream(StreamId::new(id, media_type))?;
                        self.talk.layout::<Layout>()?;
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
            Event::SdpOffer(id, media_type, offer) => {
                log::debug!("Event::SdpOffer");
                if let Some(source) = self.talk.get_source(&StreamId::new(id, media_type)) {
                    let answer = source.receive_offer(offer).await?;
                    self.signaling.send_answer(id, media_type, answer).await?;
                }
            }
            Event::SdpCandidate(id, media_type, candidate) => {
                log::debug!("Event::SdpCandidate");
                if let Some(source) = self.talk.get_source(&StreamId::new(id, media_type)) {
                    source
                        .receive_candidate(candidate.sdp_m_line_index as u32, candidate.candidate)
                        .await;
                }
            }
            Event::SdpEndOfCandidates(id, media_type) => {
                log::debug!("Event::SdpEndOfCandidates");
                let state = &self.signaling.participants()[&id];
                if state.publishes(&media_type).is_some() {
                    if let Some(source) = self.talk.get_source(&StreamId::new(id, media_type)) {
                        source.receive_end_of_candidates(0).await;
                    }
                }
            }
            Event::FocusUpdate(focus_change) => {
                log::debug!("Event::FocusUpdate");
                log::debug!("Set active speaker to {:?}", focus_change);
                if let Some(speaker) = focus_change {
                    self.talk
                        .set_speaker(Some(speaker), &compositor::SpeakerSwitchMode::FirstShift)?;
                } else {
                    self.talk
                        .set_speaker(None, &compositor::SpeakerSwitchMode::FirstShift)?;
                }
                self.talk.layout::<Layout>()?;
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
        (id, mline, candidate): (ParticipantId, u32, Option<String>),
    ) -> Result<()> {
        if let Some(candidate) = candidate {
            for media_type in media_types() {
                self.signaling
                    .send_candidate(
                        id,
                        media_type,
                        TrickleCandidate {
                            candidate: candidate.clone(),
                            sdp_m_line_index: mline as u64,
                        },
                    )
                    .await?
            }
        } else {
            for media_type in media_types() {
                self.signaling
                    .send_end_of_candidates(id, media_type)
                    .await?
            }
        }
        Ok(())
    }

    async fn upload(self) -> Result<()> {
        let talk = self.talk;

        spawn_blocking(move || drop(talk)).await?;

        let recording_path = self.temp_dir.path().join("out.mp4");

        if let Err(upload_err) = self
            .service_context
            .upload(&self.room_id, recording_path.as_ref())
            .await
        {
            let dump_name = "DUMP.mp4";
            error!(
                "upload of file {:?} failed. Saving output in {dump_name}.",
                recording_path
            );
            tokio::fs::copy(recording_path, dump_name).await?;
            return Err(upload_err);
        }

        log::debug!("Finished uploading recording for room '{}'", &self.room_id);

        Ok(())
    }
}

fn participant_params(
    id: ParticipantId,
    sender: mpsc::Sender<(ParticipantId, u32, Option<String>)>,
) -> WebRtcSourceParams {
    WebRtcSourceParams::default().on_ice_candidate(move |mline, candidate| {
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

        if read_buf.filled().is_empty() {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(Bytes::copy_from_slice(read_buf.filled()))))
        }
    }
}
