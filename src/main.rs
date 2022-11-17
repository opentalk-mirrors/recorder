use crate::http::HttpClient;
use crate::signaling::MediaSessionType;
use anyhow::Result;
use bytes::Bytes;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use futures::future::join_all;
use futures::{Stream, StreamExt};
use gst::glib;
use settings::Settings;
use signaling::ParticipantId;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncRead, ReadBuf};
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
    record(
        settings.clone(),
        http_client.clone(),
        commands::StartRecording {
            room: "52657198-e121-4347-9335-d5a26dd31c50".into(),
            breakout: None,
        },
    )
    .await;

    // TODO: this grows into infinity
    let mut tasks = vec![];

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                if let Ok(command) =
                    serde_json::from_slice::<commands::StartRecording>(&delivery.data)
                {
                    log::debug!("Received command {command:?}");

                    tasks.push(tokio::spawn(record(
                        settings.clone(),
                        http_client.clone(),
                        command,
                    )));
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

type Mixer = compositor::Mixer<
    compositor::Speaker,
    compositor::WebRtcSource,
    compositor::Mp4Sink,
    ParticipantId,
>;

async fn record(
    settings: Arc<Settings>,
    http_client: Arc<HttpClient>,
    command: commands::StartRecording,
) {
    let mut signaling =
        signaling::Signaling::connect(http_client.clone(), settings.clone(), &command.room)
            .await
            .unwrap();

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("out.mp4");

    let mut mixer = Mixer::new(
        compositor::Size::FHD,
        6,
        compositor::Mp4SinkParams {
            file_path: file_path.to_str().unwrap().into(),
        },
    )
    .unwrap();

    let mut list = HashSet::new();

    for id in signaling.publishing_participants() {
        mixer.add_participant(id, id.0.to_string(), ()).unwrap();
        signaling
            .start_subscribe(id, MediaSessionType::Video)
            .await
            .unwrap();
        list.insert(id);
    }

    mixer.play();

    loop {
        let event = match signaling.run().await {
            Ok(event) => event,
            Err(e) => {
                log::error!("signaling error {:?}", e);
                return;
            }
        };

        match event {
            signaling::Event::ParticipantJoined(id) => {
                if signaling.publishes(id, MediaSessionType::Video) {
                    mixer.pause();
                    mixer.add_participant(id, id.0.to_string(), ()).unwrap();
                    mixer.play();

                    signaling
                        .start_subscribe(id, MediaSessionType::Video)
                        .await
                        .unwrap();
                    list.insert(id);
                }
            }
            signaling::Event::ParticipantUpdated(id) => {
                if !list.contains(&id) && signaling.publishes(id, MediaSessionType::Video) {
                    mixer.pause();
                    mixer.add_participant(id, id.0.to_string(), ()).unwrap();
                    mixer.play();

                    signaling
                        .start_subscribe(id, MediaSessionType::Video)
                        .await
                        .unwrap();
                    list.insert(id);
                }
            }
            signaling::Event::ParticipantLeft(id) => {
                if list.remove(&id) {
                    mixer.pause();

                    mixer.remove_participant(id).unwrap();

                    mixer
                        .set_visibles(&list.iter().copied().collect::<Vec<_>>())
                        .unwrap();

                    mixer.play();
                }

                // Finish recording when the last participant leaves
                if list.is_empty() {
                    return finish_recording(settings, http_client, &command.room, mixer, temp_dir)
                        .await
                        .unwrap();
                }
            }
            signaling::Event::SdpOffer(id, typ, offer) => {
                mixer.pause();

                let answer = mixer.participants[&id].source.receive_offer(offer).await;

                mixer
                    .set_visibles(&list.iter().copied().collect::<Vec<_>>())
                    .unwrap();

                mixer.play();

                signaling.send_answer(id, typ, answer).await.unwrap();
            }
            signaling::Event::SdpCandidate(id, _typ, candidate) => todo!(),
            signaling::Event::SdpEndOfCandidates(id, _typ) => {
                if let Some(participant) = mixer.participants.get_mut(&id) {}
            }
        }
    }
}

async fn finish_recording(
    settings: Arc<Settings>,
    http_client: Arc<HttpClient>,
    room_id: &str,
    mixer: Mixer,
    temp_dir: TempDir,
) -> Result<()> {
    // Drop mixer in a separate thread to avoid blocking tokio while it waits for the EOS event and ffmpeg to exit
    spawn_blocking(move || drop(mixer)).await?;

    let file = tokio::fs::File::open(temp_dir.path().join("out.mp4")).await?;

    log::trace!("upload mp4 file");

    http_client
        .upload_render(&settings.controller, room_id, FileReadStream { file })
        .await?;

    log::trace!("finished uploading mp4 file");

    Ok(())
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
