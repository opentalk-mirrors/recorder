use anyhow::Result;
use futures::future::join_all;
use futures::StreamExt;
use gst::glib;
use settings::Settings;
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

    let mut mixer = Mixer::<Speaker, TestSource>::new::<DisplaySink>(
        // resolution
        &Size {
            width: 1920,
            height: 1080,
        },
        // partcipants
        20,
        // maximum visibles
        6,
    )
    .unwrap();

    mixer.play();

    let names: Vec<String> = vec!["Peer", "Markus", "Michael", "Konstantin", "Pat"]
        .iter()
        .map(|n| n.to_string())
        .collect();
    mixer.add_participants(&names);

    loop {
        let event = match signaling.run().await {
            Ok(event) => event,
            Err(e) => {
                log::error!("signaling error {:?}", e);
                return;
            }
        };

        match event {
            signaling::Event::ParticipantJoined(id) => mixer.add_participants(&[id]),
            signaling::Event::ParticipantUpdated(id) => todo!(),
            signaling::Event::ParticipantLeft(id) => todo!(),
            signaling::Event::SdpOffer(id, typ, offer) => {}
            signaling::Event::SdpCandidate(id, typ, candidate) => todo!(),
            signaling::Event::SdpEndOfCandidates(id, typ) => {}
        }
    }
}
