use crate::signaling::MediaSessionType;
use anyhow::Result;
use futures::future::join_all;
use futures::StreamExt;
use gst::glib;
use settings::Settings;
use std::collections::HashSet;
use std::sync::Arc;

mod commands;
mod settings;
mod signaling;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    gst::init()?;

    let settings = Arc::new(Settings::load("config.toml")?);

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

    let http_client = reqwest::Client::new();

    // TODO: remove me!
    record(
        settings.clone(),
        http_client.clone(),
        commands::StartRecording {
            room: "f5c0099c-4645-4162-b89c-9b0aeba01600".into(),
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
                log::error!("rabbitmq consumer returned error: {}", e);
                break;
            }
        }
    }

    log::info!("Exiting, waiting for all tasks to finish");

    join_all(tasks).await;

    log::info!("Exiting, waiting for all tasks to finish");

    Ok(())
}

async fn record(
    settings: Arc<Settings>,
    http_client: reqwest::Client,
    command: commands::StartRecording,
) {
    let mut signaling = signaling::Signaling::connect(http_client, settings.clone(), command.room)
        .await
        .unwrap();

    use compositor::*;

    let mut mixer = Mixer::<Speaker, WebRtcSource>::new::<DisplaySink>(
        // resolution
        Size {
            width: 1920,
            height: 1080,
        },
        // partcipants
        20,
        // maximum visibles
        6,
    )
    .unwrap();

    let mut list = HashSet::new();

    for id in signaling.publishing_participants() {
        mixer.add_participant(id.0.to_string(), ()).unwrap();
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
                    mixer.add_participant(id.0.to_string(), ()).unwrap();
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
                    mixer.add_participant(id.0.to_string(), ()).unwrap();
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

                    mixer.remove_participant(&id.0.to_string()).unwrap();

                    mixer
                        .set_visibles(
                            &list
                                .iter()
                                .map(|id| id.0.to_string())
                                .collect::<Vec<String>>(),
                        )
                        .unwrap();

                    mixer.play();
                }
            }
            signaling::Event::SdpOffer(id, typ, offer) => {
                mixer.pause();

                let answer = mixer.participants[&id.0.to_string()]
                    .source
                    .receive_offer(offer)
                    .await;

                mixer
                    .set_visibles(
                        &list
                            .iter()
                            .map(|id| id.0.to_string())
                            .collect::<Vec<String>>(),
                    )
                    .unwrap();

                mixer.play();

                signaling.send_answer(id, typ, answer).await.unwrap();
            }
            signaling::Event::SdpCandidate(_id, _typ, _candidate) => todo!(),
            signaling::Event::SdpEndOfCandidates(_id, _typ) => {}
        }
    }
}
