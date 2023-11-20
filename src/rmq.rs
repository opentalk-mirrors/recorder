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
pub struct StartRecording {
    pub room: String,
    pub breakout: Option<String>,
}

pub async fn connect_rabbitmq(settings: &RabbitMqSettings) -> Result<Consumer> {
    let rmq_conn = lapin::Connection::connect_uri(
        settings.uri.clone(),
        lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio),
    )
    .await?;

    let rmq_channel = rmq_conn.create_channel().await?;

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

pub async fn handle_delivery(delivery: &Delivery) -> Result<StartRecording> {
    delivery
        .ack(BasicAckOptions::default())
        .await
        .context("failed to ACK")?;

    serde_json::from_slice::<StartRecording>(&delivery.data)
        .with_context(|| format!("Failed to parse RMQ message {:?}", &delivery.data))
}
