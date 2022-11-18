use crate::http::HttpClient;
use crate::signaling::MediaSessionType;
use anyhow::Result;
use bytes::Bytes;
use compositor::WebRtcSourceParams;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use futures::future::join_all;
use futures::{Stream, StreamExt};
use gst::glib;
use settings::Settings;
use signaling::{ParticipantId, TrickleCandidate};
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

mod commands;
mod http;
mod settings;
mod signaling;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    gst::init()?;

    let settings = Arc::new(Settings::load("config.toml")?);

    let http_client = Arc::new(HttpClient::discover(&settings.auth).await?);

    let _main_loop = glib::MainLoop::new(None, false);

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

    let mut consumer = rmq_channel
        .basic_consume(
            queue.name().as_str(),
            "",
            Default::default(),
            Default::default(),
        )
        .await?;

    // TODO: remove me!
    RecordingSession::create(
        settings.clone(),
        http_client.clone(),
        commands::StartRecording {
            room: "56b4287f-7cb6-4728-bf91-294019362dc4".into(),
            breakout: None,
        },
    )
    .await
    .unwrap()
    .run()
    .await
    .unwrap();

    // TODO: this grows into infinity
    let mut tasks = vec![];

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                if let Ok(command) =
                    serde_json::from_slice::<commands::StartRecording>(&delivery.data)
                {
                    log::debug!("Received command {command:?}");
                    let settings = settings.clone();
                    let http_client = http_client.clone();

                    tasks.push(tokio::spawn(async move {
                        RecordingSession::create(settings, http_client, command)
                            .await
                            .unwrap()
                            .run()
                            .await
                            .unwrap();
                    }));
                }
            }
            Err(e) => {
                log::error!("RabbitMQ consumer returned error: {}", e);
                break;
            }
        }
    }

    log::info!("Exiting, waiting for all tasks to finish");

    join_all(tasks).await;

    log::info!("Exiting, waiting for all tasks to finish");

    Ok(())
}

// TODO; make this configurable
const MAX_VISIBLES: usize = 6;

type Mixer = compositor::Mixer<
    compositor::Speaker,
    compositor::WebRtcSource,
    compositor::Mp4Sink,
    ParticipantId,
>;

pub struct RecordingSession {
    settings: Arc<Settings>,
    http_client: Arc<http::HttpClient>,
    signaling: signaling::Signaling,

    room_id: String,
    temp_dir: TempDir,

    mixer: Mixer,

    candidate_receiver: mpsc::Receiver<(ParticipantId, u32, Option<String>)>,
    candidate_sender: mpsc::Sender<(ParticipantId, u32, Option<String>)>,

    // TODO: remove this: VVV
    subscribed_participants: HashSet<ParticipantId>,

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

        let mut mixer = Mixer::new(
            compositor::Size::FHD,
            MAX_VISIBLES,
            compositor::Mp4SinkParams {
                file_path: file_path.to_str().unwrap().into(),
            },
        )?;

        let mut subscribed_participants = HashSet::new();

        // find all participants that publish their webcam
        let publishing_participants = signaling
            .participants()
            .iter()
            .filter_map(|(id, state)| {
                state
                    .publishes(MediaSessionType::Video)
                    .then(|| (*id, state.display_name.clone()))
            })
            .collect::<Vec<_>>();

        // Subscribe to above collected participants
        for (id, display_name) in publishing_participants {
            mixer.add_participant(
                id,
                display_name,
                participant_params(id, candidate_sender.clone()),
            )?;
            signaling
                .start_subscribe(id, MediaSessionType::Video)
                .await?;
            subscribed_participants.insert(id);
        }

        mixer.play();

        Ok(Self {
            settings,
            http_client,
            signaling,
            room_id: command.room,
            temp_dir,
            mixer,
            candidate_receiver,
            candidate_sender,
            subscribed_participants,
            done: false,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        while !self.done {
            tokio::select! {
                event = self.signaling.run() => {
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

    async fn handle_signaling_event(&mut self, event: signaling::Event) -> Result<()> {
        match event {
            signaling::Event::ParticipantJoined(id) => {
                let state = &self.signaling.participants()[&id];

                if state.publishes(MediaSessionType::Video) {
                    self.mixer.pause();
                    self.mixer.add_participant(
                        id,
                        id.0.to_string(),
                        participant_params(id, self.candidate_sender.clone()),
                    )?;
                    self.mixer.play();

                    self.signaling
                        .start_subscribe(id, MediaSessionType::Video)
                        .await?;
                    self.subscribed_participants.insert(id);
                }
            }
            signaling::Event::ParticipantUpdated(id) => {
                let state = &self.signaling.participants()[&id];

                if state.publishes(MediaSessionType::Video)
                    && !self.subscribed_participants.contains(&id)
                {
                    // Now publishing
                    self.mixer.pause();
                    self.mixer.add_participant(
                        id,
                        id.0.to_string(),
                        participant_params(id, self.candidate_sender.clone()),
                    )?;
                    self.mixer.play();

                    self.signaling
                        .start_subscribe(id, MediaSessionType::Video)
                        .await?;
                    self.subscribed_participants.insert(id);
                } else if self.subscribed_participants.contains(&id)
                    && !state.publishes(MediaSessionType::Video)
                {
                    // Unpublished
                    self.subscribed_participants.remove(&id);

                    self.mixer.pause();
                    self.mixer.remove_participant(id)?;
                    self.set_visibles()?;
                    self.mixer.play();
                }
            }
            signaling::Event::ParticipantLeft(id) => {
                if self.subscribed_participants.remove(&id) {
                    self.mixer.pause();
                    self.mixer.remove_participant(id)?;
                    self.set_visibles()?;
                    self.mixer.play();
                }

                if self.signaling.participants().is_empty() {
                    self.done = true;
                }
            }
            signaling::Event::SdpOffer(id, typ, offer) => {
                if let Some(participant) = self.mixer.participants.get_mut(&id) {
                    let answer = participant.source.receive_offer(offer).await;
                    self.signaling.send_answer(id, typ, answer).await?;
                }
            }
            signaling::Event::SdpCandidate(id, _typ, candidate) => {
                if let Some(participant) = self.mixer.participants.get_mut(&id) {
                    participant
                        .source
                        .receive_candidate(candidate.sdp_m_line_index as u32, candidate.candidate)
                        .await;
                }
            }
            signaling::Event::SdpEndOfCandidates(id, _typ) => {
                if let Some(participant) = self.mixer.participants.get_mut(&id) {
                    participant.source.receive_end_of_candidates(0).await;
                }
            }
            signaling::Event::Close => self.done = true,
        }

        Ok(())
    }

    fn set_visibles(&mut self) -> Result<()> {
        self.mixer.set_visibles(
            &self
                .subscribed_participants
                .iter()
                .take(MAX_VISIBLES)
                .copied()
                .collect::<Vec<_>>(),
        )?;
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
                    MediaSessionType::Video,
                    TrickleCandidate {
                        candidate,
                        sdp_m_line_index: mline as u64,
                    },
                )
                .await
        } else {
            self.signaling
                .send_end_of_candidates(id, MediaSessionType::Video)
                .await
        }
    }

    async fn upload(self) -> Result<()> {
        let mixer = self.mixer;

        spawn_blocking(move || drop(mixer)).await?;

        let file = tokio::fs::File::open(self.temp_dir.path().join("out.mp4")).await?;

        log::trace!("upload mp4 file");

        self.http_client
            .upload_render(
                &self.settings.controller,
                &self.room_id,
                FileReadStream { file },
            )
            .await?;

        log::trace!("finished uploading mp4 file");

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
