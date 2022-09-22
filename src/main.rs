use std::sync::Arc;

use anyhow::Result;
use futures::{future::join_all, StreamExt};
use gst::glib;
use settings::Settings;

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
            .with_executor(tokio_executor_trait::Tokio::current()) // TODO: contribute to https://github.com/amqp-rs/executor-trait
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

    handle_command(
        http_client.clone(),
        settings.clone(),
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
                    tasks.push(tokio::spawn(handle_command(
                        http_client.clone(),
                        settings.clone(),
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

async fn handle_command(
    http_client: reqwest::Client,
    settings: Arc<Settings>,
    command: commands::StartRecording,
) {
    signaling::Signaling::connect(http_client, settings.clone(), command.room)
        .await
        .unwrap();
}
