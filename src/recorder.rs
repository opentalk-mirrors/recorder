use crate::http::HttpClient;
use crate::rmq::StartRecording;
use crate::settings::Settings;
use crate::signaling::incoming::MediaSessionState;
use crate::signaling::{media_types, Event, Signaling};
use crate::signaling::{ParticipantId, TrickleCandidate};
use anyhow::{bail, Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{MediaSessionType, SinkBuilder, StreamId, WebRtcSourceParams};
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

#[derive(Clone, Debug)]
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
            let mut session = match RecordingSession::create(&context, command).await {
                Ok(session) => session,
                Err(error) => {
                    error!("Failed to start RecordingSession: {:?}", error);
                    return;
                }
            };

            if let Err(ref recording_err) = session.run().await {
                error!(
                    "recording session failed but trying upload anyway {:?}",
                    recording_err
                );
            };
            if let Err(ref upload_err) = session.upload().await {
                error!("recording upload failed {:?}", upload_err);
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

#[derive(Debug)]
pub struct RecordingSession<'a> {
    service_context: &'a Recorder,

    signaling: Signaling,

    room_id: String,
    temp_dir: TempDir,

    talk: Talk,

    candidate_receiver: mpsc::Receiver<(StreamId<ParticipantId>, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(StreamId<ParticipantId>, u32, Option<String>)>,

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

        let sink_setting = service_context
            .settings
            .recorder
            .as_ref()
            .map(|rec| rec.sink.as_str());

        let sink_builder: Box<dyn SinkBuilder + Send> = match sink_setting {
            Some("display") => Box::<compositor::DisplaySinkBuilder>::default(),
            Some("matroska") => {
                let params = service_context
                    .settings
                    .matroska
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                Box::new(compositor::MatroskaSinkBuilder::new(params))
            }
            _ => Box::new(compositor::Mp4SinkBuilder::new(compositor::Mp4SinkParams {
                file_path: file_path
                    .to_str()
                    .expect("failed to convert MP4 file path into string")
                    .into(),
            })),
        };
        let mut talk = Talk::new(compositor::Size::FHD, sink_builder, Some(MAX_VISIBLES))?;

        // find all active media streams
        let available_media_streams: Vec<(
            ParticipantId,
            String,
            MediaSessionType,
            MediaSessionState,
        )> = signaling
            .participants()
            .iter()
            .flat_map(|(id, participant_state)| {
                media_types().filter_map(|media_type| {
                    participant_state.publishes(&media_type).map(|media_state| {
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
            let stream_id = StreamId::new(id, media_type);
            talk.add_stream(
                stream_id,
                &display_name,
                stream_params(stream_id, candidate_sender.clone()),
                media_state.into(),
            )?;
            talk.layout::<Layout>()?;
            signaling.start_subscribe(stream_id).await?;
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

    pub async fn run(&mut self) -> Result<()> {
        while !self.done {
            tokio::select! {
                event = self.signaling.run() => {
                    let signaling_msg = event.context("signaling error")?;
                    log::trace!("signaling_event {:?}", signaling_msg);
                    self.handle_signaling_event(signaling_msg).await?;
                }
                maybe_candidate = self.candidate_receiver.recv() => {
                    let Some((stream_id, mline, candidate)) = maybe_candidate else {
                        bail!("no candidate pair found");
                    };
                    self.handle_candidate(stream_id, mline, candidate).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_signaling_event(&mut self, event: Event) -> Result<()> {
        log::debug!("handle_signaling_event");
        match event {
            Event::ParticipantJoined(id) => {
                log::debug!("Event::ParticipantJoined");

                let participant_state = self.signaling.participant(&id)?.clone();
                let available_media_streams = media_types().filter_map(|media_type| {
                    participant_state
                        .publishes(&media_type)
                        .map(|media_state| (media_type, media_state))
                });

                for (media_type, media_state) in available_media_streams {
                    log::debug!("Join: subscribe stream of {id} {media_type}");
                    let stream_id = StreamId::new(id, media_type);
                    self.talk.add_stream(
                        stream_id,
                        &participant_state.display_name,
                        stream_params(stream_id, self.candidate_sender.clone()),
                        media_state.into(),
                    )?;
                    self.talk.layout::<Layout>()?;
                    self.signaling.start_subscribe(stream_id).await?;
                }
            }
            Event::ParticipantUpdated(id) => {
                log::debug!("Event::ParticipantUpdated");
                let participant_state = self.signaling.participant(&id)?.clone();

                for media_type in media_types() {
                    let is_subscribed = self.talk.contains_stream(&StreamId::new(id, media_type));
                    let media_state = participant_state.publishes(&media_type);

                    if !is_subscribed {
                        if let Some(media_state) = media_state {
                            log::debug!("Update: subscribe stream of {id} {media_type}");
                            let stream_id = StreamId::new(id, media_type);
                            self.talk.add_stream(
                                stream_id,
                                &participant_state.display_name,
                                stream_params(stream_id, self.candidate_sender.clone()),
                                media_state.into(),
                            )?;
                            self.signaling.start_subscribe(stream_id).await?;
                            if participant_state.consents {
                                self.talk.show(&stream_id)?;
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
            Event::SdpOffer(stream_id, offer) => {
                log::debug!("Event::SdpOffer");
                if let Some(source) = self.talk.get_source(&stream_id) {
                    let answer = source.receive_offer(offer).await?;
                    self.signaling.send_answer(stream_id, answer).await?;
                }
            }
            Event::SdpCandidate(stream_id, candidate) => {
                log::debug!("Event::SdpCandidate");
                if let Some(source) = self.talk.get_source(&stream_id) {
                    source
                        .receive_candidate(candidate.sdp_m_line_index as u32, candidate.candidate)
                        .await;
                }
            }
            Event::SdpEndOfCandidates(stream_id) => {
                log::debug!("Event::SdpEndOfCandidates");
                let participant_state = self.signaling.participant(&stream_id.id)?;

                if participant_state.publishes(&stream_id.media_type).is_none() {
                    bail!(
                        "EndOfCandidates message for {:?} with no media stream",
                        stream_id
                    );
                }
                let Some(source) = self.talk.get_source(&stream_id) else {
                    bail!("EndOfCandidates message for {:?} with no connection setup", stream_id);
                };

                source.receive_end_of_candidates(0).await;
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
        stream_id: StreamId<ParticipantId>,
        mline: u32,
        candidate: Option<String>,
    ) -> Result<()> {
        if let Some(candidate) = candidate {
            self.signaling
                .send_candidate(
                    stream_id,
                    TrickleCandidate {
                        candidate: candidate.clone(),
                        sdp_m_line_index: mline as u64,
                    },
                )
                .await?
        } else {
            self.signaling.send_end_of_candidates(stream_id).await?
        }
        Ok(())
    }

    async fn upload(self) -> Result<()> {
        let talk = self.talk;
        spawn_blocking(move || drop(talk)).await?;

        let recording_path = self.temp_dir.path().join("out.mp4");

        let Err(upload_err) = self
            .service_context
            .upload(&self.room_id, recording_path.as_ref())
            .await else
        {
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
    id: StreamId<ParticipantId>,
    sender: mpsc::Sender<(StreamId<ParticipantId>, u32, Option<String>)>,
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

        let buffer = read_buf.filled();
        if buffer.is_empty() {
            return Poll::Ready(None);
        }

        Poll::Ready(Some(Ok(Bytes::copy_from_slice(buffer))))
    }
}
