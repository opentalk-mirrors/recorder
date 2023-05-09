use crate::http::HttpClient;
use crate::signaling::{Event, MediaSessionType};
use anyhow::{Context as ErrorContext, Result};
use bytes::Bytes;
use compositor::{StreamStatus, WebRtcSourceParams};
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use futures::future::join_all;
use futures::{Stream, StreamExt};
use gst::glib;
use lapin::message::Delivery;
use lapin::Consumer;
use log::error;
use settings::Settings;
use signaling::{ParticipantId, TrickleCandidate};
use std::io;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::task::{spawn_blocking, JoinHandle};
use tokio::time::{sleep, Duration};

mod commands;
mod http;
mod settings;
mod signaling;

const RECONNECT_INTERVAL: Duration = Duration::from_millis(3_000); //ms

fn main() -> Result<()> {
    env_logger::init();
    gst::init()?;

    let main_loop = glib::MainLoop::new(None, false);

    let main_loop_clone = main_loop.clone();
    let _recorder = std::thread::spawn(move || {
        let res = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to start tokio async runtime")
            .block_on(main2());

        if let Err(e) = res {
            eprintln!("Exit on failure: {:?}", e);
            std::process::exit(-1);
        }
        main_loop_clone.quit();
    });

    main_loop.run();

    Ok(())
}

async fn connect_rabbitmq(settings: &Settings) -> Result<Consumer> {
    let rmq_conn = lapin::Connection::connect_uri(
        settings.rabbitmq.uri.clone(),
        lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio),
    )
    .await?;

    let rmq_channel = rmq_conn.create_channel().await?;

    let queue = rmq_channel
        .queue_declare(
            &settings.rabbitmq.queue,
            Default::default(),
            Default::default(),
        )
        .await?;

    rmq_channel
        .basic_consume(
            queue.name().as_str(),
            "",
            Default::default(),
            Default::default(),
        )
        .await
        .context("Failed to create consumer for RMQ channel")
}

async fn handle_rabbitmq_delivery(
    delivery: &Delivery,
    http_client: Arc<HttpClient>,
    settings: Arc<Settings>,
) -> Result<JoinHandle<()>> {
    delivery
        .ack(Default::default())
        .await
        .context("failed to ACK")?;

    let command = serde_json::from_slice::<commands::StartRecording>(&delivery.data)
        .with_context(|| format!("Failed to parse RMQ message {:?}", &delivery.data))?;

    log::debug!("Received start command ({command:?})");

    let recording_task = tokio::spawn(async move {
        let session_result = RecordingSession::create(settings, http_client, command)
            .await
            .expect("Failed to start signaling session")
            .run()
            .await;
        if let Err(e) = session_result {
            log::error!("Recording session failed: {:?}", e);
        }
    });

    Ok(recording_task)
}

async fn main2() -> Result<()> {
    let settings = Arc::new(Settings::load("config.toml")?);
    let http_client = Arc::new(HttpClient::discover(&settings.auth).await?);
    let mut tasks = vec![];

    // TODO react to SIGTERM
    loop {
        match connect_rabbitmq(&settings).await {
            Ok(mut consumer) => {
                while let Some(delivery) = consumer.next().await {
                    match delivery {
                        Ok(ref delivery) => {
                            let task = handle_rabbitmq_delivery(
                                delivery,
                                http_client.clone(),
                                settings.clone(),
                            )
                            .await?;
                            tasks.push(task);
                        }
                        Err(e) => {
                            log::error!("RabbitMQ consumer returned error: {}", e);
                            break;
                        }
                    }
                    tasks.retain(|task| !task.is_finished());
                }
            }
            Err(e) => {
                log::error!("RMQ connect error: {:?}", e);
                tasks.retain(|task| !task.is_finished());
                if tasks.is_empty() {
                    log::info!("Exiting after error as all tasks are finished");
                    break;
                }
                log::info!("Retry RMQ connect in {:?}", RECONNECT_INTERVAL);
                sleep(RECONNECT_INTERVAL).await;
            }
        }
    }

    join_all(tasks).await;

    Ok(())
}

// TODO; make this configurable
const MAX_VISIBLES: usize = 6;

type Talk = compositor::Talk<compositor::WebRtcSource, ParticipantId>;
type Layout = compositor::Grid;

pub struct RecordingSession {
    settings: Arc<Settings>,
    http_client: Arc<HttpClient>,
    signaling: signaling::Signaling,

    room_id: String,
    temp_dir: TempDir,

    talk: Talk,

    candidate_receiver: mpsc::Receiver<(ParticipantId, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(ParticipantId, u32, Option<String>)>,

    done: bool,
}

impl RecordingSession {
    pub async fn create(
        settings: Arc<Settings>,
        http_client: Arc<HttpClient>,
        command: commands::StartRecording,
    ) -> Result<Self> {
        let mut signaling =
            signaling::Signaling::connect(http_client.clone(), settings.clone(), &command.room)
                .await?;

        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("out.mp4");

        let (candidate_sender, candidate_receiver) = mpsc::channel(12);

        let mut mixer = Talk::new(
            compositor::Size::FHD,
            if let Some(recorder) = &settings.recorder {
                match recorder.sink.to_lowercase().as_str() {
                    "display" => Box::new(compositor::DisplaySinkBuilder::new()),
                    "matroska" => {
                        if let Some(params) = &settings.matroska {
                            Box::new(compositor::MatroskaSinkBuilder::new(params.clone()))
                        } else {
                            Box::new(compositor::MatroskaSinkBuilder::new(Default::default()))
                        }
                    }
                    "mp4" | _ => {
                        Box::new(compositor::Mp4SinkBuilder::new(compositor::Mp4SinkParams {
                            file_path: file_path
                                .to_str()
                                .expect("failed to convert MP4 file path into string")
                                .into(),
                        }))
                    }
                }
            } else {
                todo!()
            },
            Some(MAX_VISIBLES),
        )?;

        // find all participants that publish their webcam
        let publishing_participants = signaling
            .participants()
            .iter()
            .filter_map(|(id, state)| {
                state
                    .publishes(MediaSessionType::Camera)
                    .is_some()
                    .then(|| {
                        (
                            *id,
                            state.display_name.clone(),
                            state.publishes(MediaSessionType::Camera).unwrap(),
                        )
                    })
            })
            .collect::<Vec<_>>();

        // Subscribe to above collected participants
        for (id, display_name, initial) in publishing_participants {
            mixer.add_participant(
                id.into(),
                display_name,
                participant_params(id, candidate_sender.clone()),
                initial.into(),
            )?;
            signaling
                .start_subscribe(id, MediaSessionType::Camera)
                .await?;
        }

        mixer.layout::<Layout>()?;

        Ok(Self {
            settings,
            http_client,
            signaling,
            room_id: command.room,
            temp_dir,
            talk: mixer,
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
        match event {
            Event::ParticipantJoined(id) => {
                let state = &self.signaling.participants()[&id];
                let media_state = state.publishes(MediaSessionType::Camera);

                if let Some(media_state) = media_state {
                    log::debug!("Join: subscribe Video of {:?}", id);
                    self.talk.add_participant(
                        id.into(),
                        state.display_name.to_string(),
                        participant_params(id, self.candidate_sender.clone()),
                        StreamStatus {
                            has_audio: media_state.audio,
                            has_video: media_state.video,
                        },
                    )?;
                    self.talk.layout::<Layout>()?;
                    self.signaling
                        .start_subscribe(id, MediaSessionType::Camera)
                        .await?;
                }
            }
            Event::ParticipantUpdated(id) => {
                let state = &self.signaling.participants()[&id];
                let media_state = state.publishes(MediaSessionType::Camera);
                let is_subscribed = self.talk.contains_stream(&id.into());

                if !is_subscribed {
                    if let Some(media_state) = media_state {
                        log::debug!("Update: subscribe Video of {:?}", id);
                        self.talk.add_participant(
                            id.into(),
                            state.display_name.to_string(),
                            participant_params(id, self.candidate_sender.clone()),
                            StreamStatus {
                                has_audio: media_state.audio,
                                has_video: media_state.video,
                            },
                        )?;
                        self.talk.layout::<Layout>()?;
                        self.signaling
                            .start_subscribe(id, MediaSessionType::Camera)
                            .await?;
                        return Ok(());
                    }
                }

                if is_subscribed && media_state.is_none() {
                    log::debug!("Update: unsubscribe Video of {:?}", id);
                    self.talk.remove_stream(id.into())?;
                    self.talk.layout::<Layout>()?;
                    return Ok(());
                }

                if let Some(media_state) = media_state {
                    self.talk.set_status(
                        &id.into(),
                        StreamStatus {
                            has_audio: media_state.audio,
                            has_video: media_state.video,
                        },
                    )?;
                    self.talk.layout::<Layout>()?;
                }

                log::trace!(
                    "ignore update for {:?}: has_video_feed ({:?}) == is_subscribed ({})",
                    id,
                    media_state,
                    is_subscribed
                );

                return Ok(());
            }
            Event::ParticipantLeft(id) => {
                if self.talk.contains_stream(&id.into()) {
                    self.talk.remove_stream(id.into())?;
                    self.talk.layout::<Layout>()?;
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
            Event::SdpOffer(id, typ, offer) => {
                if let Some(source) = self.talk.get_source(&id.into()) {
                    let answer = source.receive_offer(offer).await;
                    self.signaling.send_answer(id, typ, answer).await?;
                }
            }
            Event::SdpCandidate(id, _typ, candidate) => {
                if let Some(source) = self.talk.get_source(&id.into()) {
                    source
                        .receive_candidate(candidate.sdp_m_line_index as u32, candidate.candidate)
                        .await;
                }
            }
            Event::SdpEndOfCandidates(id, _typ) => {
                if let Some(source) = self.talk.get_source(&id.into()) {
                    source.receive_end_of_candidates(0).await;
                }
            }
            Event::FocusUpdate(focus_change) => {
                log::debug!("Set active speaker to {:?}", focus_change);
                if let Some(speaker) = focus_change {
                    self.talk.set_speaker(
                        Some(speaker.into()),
                        &compositor::SpeakerSwitchMode::FirstShift,
                    )?;
                } else {
                    self.talk
                        .set_speaker(None, &compositor::SpeakerSwitchMode::FirstShift)?;
                }
                self.talk.layout::<Layout>()?;
            }
            Event::MediaConnectionError(error) => {
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
            self.signaling
                .send_candidate(
                    id,
                    MediaSessionType::Camera,
                    TrickleCandidate {
                        candidate,
                        sdp_m_line_index: mline as u64,
                    },
                )
                .await
        } else {
            self.signaling
                .send_end_of_candidates(id, MediaSessionType::Camera)
                .await
        }
    }

    async fn upload(self) -> Result<()> {
        let mixer = self.talk;

        spawn_blocking(move || drop(mixer)).await?;

        let recording_path = self.temp_dir.path().join("out.mp4");
        let file = tokio::fs::File::open(&recording_path).await?;

        log::debug!(
            "upload mp4 file '{:?}' for room: {}",
            recording_path,
            &self.room_id
        );

        match self
            .http_client
            .upload_render(
                &self.settings.controller,
                &self.room_id,
                FileReadStream { file },
            )
            .await
        {
            Ok(_) => (),
            Err(err) => {
                let dump_name = "DUMP.mp4";
                error!(
                    "upload of file {:?} failed. Saving output in {dump_name}.",
                    recording_path
                );
                tokio::fs::copy(recording_path, dump_name).await?;
                return Err(err);
            }
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
