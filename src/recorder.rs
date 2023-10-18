use crate::http::HttpClient;
use crate::rmq::StartRecording;
use crate::settings::{RecorderSettings, RecorderSink, Settings};
use crate::signaling::incoming::MediaSessionState;
use crate::signaling::{media_types, Event, Signaling};
use crate::signaling::{ParticipantId, TrickleCandidate};
use anyhow::{bail, Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{
    DisplaySink, MatroskaSink, MediaSessionType, Mp4Parameters, Mp4Sink, MultiParameters,
    MultiSink, RTMPParameters, RTMPSink, Sink, StreamId, WebRtcSourceParams,
};
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
use tokio::sync::{mpsc, watch};
use tokio::task::{spawn_blocking, JoinHandle};

// TODO; make this configurable
pub const MAX_VISIBLES: usize = 8;

type Talk = compositor::Talk<compositor::WebRtcSource, ParticipantId>;
type Layout = compositor::Speaker;

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

    pub async fn spawn_session(&self, command: StartRecording) -> Result<JoinHandle<Result<()>>> {
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

    talk: Talk,

    candidate_receiver: mpsc::Receiver<(StreamId<ParticipantId>, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(StreamId<ParticipantId>, u32, Option<String>)>,

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
        talk: Talk,
        candidate_receiver: mpsc::Receiver<(StreamId<ParticipantId>, u32, Option<String>)>,
        candidate_sender: mpsc::Sender<(StreamId<ParticipantId>, u32, Option<String>)>,
        done: bool,
    ) -> Self {
        Self {
            service_context,
            signaling,
            room_id,
            temp_dir,
            talk,
            candidate_receiver,
            candidate_sender,
            done,
        }
    }

    pub async fn create(
        service_context: Arc<Recorder>,
        command: StartRecording,
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
        let sinks = recorder_sinks
            .into_iter()
            .enumerate()
            .map::<Box<dyn Sink>, _>(|(index, sink)| {
                let tag = match sink {
                    RecorderSink::Display => "Display",
                    RecorderSink::Matroska(_) => "Matroska",
                    RecorderSink::Rtmp(_) => "RTMP",
                };
                let name = format!("{tag}-Sink-{index}");
                match sink {
                    RecorderSink::Display => Box::new(DisplaySink::new(name.as_str())),
                    RecorderSink::Matroska(matroska_parameters) => Box::new(MatroskaSink::new(
                        name.as_str(),
                        matroska_parameters.clone(),
                    )),
                    RecorderSink::Rtmp(rtmp_parameters) => Box::new(RTMPSink::new(
                        name.as_str(),
                        RTMPParameters {
                            location: rtmp_parameters.location.replace("$room", &command.room),
                            ..rtmp_parameters.clone()
                        },
                    )),
                }
            })
            .chain(std::iter::once::<Box<dyn Sink>>(Box::new(Mp4Sink::new(
                "MP4-Sink",
                Mp4Parameters {
                    file_path: file_path
                        .to_str()
                        .expect("failed to convert MP4 file path into string")
                        .into(),
                    name: "Recording",
                },
            ))))
            .collect::<Vec<_>>();

        let multi_sink = MultiSink::new(MultiParameters::new(sinks));
        let talk = Talk::new(compositor::Size::FHD, multi_sink, Some(MAX_VISIBLES))?;

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
        let mut shutdown_rx = self.service_context.shutdown.clone();

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
        stream_id: StreamId<ParticipantId>,
        display_name: &str,
        media_state: MediaSessionState,
    ) -> Result<()> {
        self.talk.add_stream(
            stream_id,
            display_name,
            stream_params(stream_id, self.candidate_sender.clone()),
            media_state.into(),
        )?;
        self.talk.layout::<Layout>()?;
        self.signaling.start_subscribe(stream_id).await?;
        // show if possible
        self.talk.try_show(&stream_id);

        Ok(())
    }

    async fn handle_signaling_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::JoinSuccess(_id) => {
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
                    self.subscribe(stream_id, &display_name, media_state)
                        .await?;
                }

                self.talk.layout::<Layout>()?;
            }

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
                    self.subscribe(stream_id, &participant_state.display_name, media_state)
                        .await?;
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
                            self.subscribe(stream_id, &participant_state.display_name, media_state)
                                .await?;
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
                    bail!(
                        "EndOfCandidates message for {:?} with no connection setup",
                        stream_id
                    );
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
