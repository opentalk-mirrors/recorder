// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context as ErrorContext, Result};
use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions},
    types::FieldTable,
    Consumer,
};
use serde::Deserialize;

use crate::settings::RabbitMqSettings;

// Commands this recorder receives via RabbitMQ

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct InitializeRecording {
    pub(crate) room: String,
    pub(crate) breakout: Option<String>,
}

pub(crate) async fn open_rabbitmq_connection(
    settings: &RabbitMqSettings,
) -> Result<lapin::Channel> {
    let rmq_conn = lapin::Connection::connect_uri(
        settings.uri.clone(),
        lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio),
    )
    .await?;

    rmq_conn
        .create_channel()
        .await
        .context("failed to create lapin::channel")
}

pub(crate) async fn create_rmq_queue_and_consume(
    rmq_channel: lapin::Channel,
    settings: &RabbitMqSettings,
) -> Result<Consumer> {
    let queue = rmq_channel
        .queue_declare(
            &settings.queue,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    rmq_channel
        .basic_consume(
            queue.name().as_str(),
            "",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .context("Failed to create consumer for RMQ channel")
}

pub(crate) async fn handle_delivery(delivery: &Delivery) -> Result<InitializeRecording> {
    delivery
        .ack(BasicAckOptions::default())
        .await
        .context("failed to ACK")?;

    serde_json::from_slice::<InitializeRecording>(&delivery.data)
        .with_context(|| format!("Failed to parse RMQ message {:?}", &delivery.data))
}
