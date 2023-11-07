// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::net::SocketAddr;

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use opentalk_recorder::signaling::{incoming, outgoing};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};
use tt::{accept_async, WebSocketStream};

pub(crate) async fn start_websocket_server(
    to_recorder_rx: mpsc::Receiver<incoming::Message>,
    to_controller_tx: mpsc::Sender<outgoing::Message>,
) -> SocketAddr {
    log::info!("Start websocket for the communication between recorder and controller");

    let (connection_tx, connection_rx) = oneshot::channel();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("unable to create tcp listener to mock the controller");

    let local_addr = listener
        .local_addr()
        .expect("unable to get local_addr from tcp listener");

    tokio::spawn(async move {
        connection_tx
            .send(())
            .expect("unable to unblock the connection_tx");
        let (connection, _) = listener.accept().await.expect("No connections to accept");
        let stream = accept_async(connection)
            .await
            .expect("Failed to handshake with connection");

        let (stream_tx, stream_rx) = stream.split();

        tokio::spawn(send_data_from_recorder_to_websocket(
            stream_tx,
            to_recorder_rx,
        ));

        tokio::spawn(send_data_from_websocket_to_controller(
            stream_rx,
            to_controller_tx,
        ));
    });

    log::debug!("Waiting for the websocket to be ready...");

    connection_rx.await.expect("Websocket not ready");

    local_addr
}

async fn send_data_from_recorder_to_websocket(
    mut stream_tx: SplitSink<WebSocketStream<TcpStream>, tt::tungstenite::Message>,
    mut to_recorder_rx: mpsc::Receiver<incoming::Message>,
) {
    while let Some(message) = to_recorder_rx.recv().await {
        log::debug!("Send message from recorder to websocket: {message:?}");
        let message = tt::tungstenite::Message::Text(
            serde_json::to_string(&message)
                .expect("unable to serialize message to tt::tungstenite::Message"),
        );
        stream_tx
            .send(message)
            .await
            .expect("unable to send data from recorder to websocket");
    }
}

async fn send_data_from_websocket_to_controller(
    mut stream_rx: SplitStream<WebSocketStream<TcpStream>>,
    to_controller_tx: mpsc::Sender<outgoing::Message>,
) {
    while let Some(message) = stream_rx.next().await {
        if let Ok(message) = message {
            let parse_result = match message {
                tt::tungstenite::Message::Text(ref s) => {
                    serde_json::from_str::<outgoing::Message>(s)
                }
                tt::tungstenite::Message::Binary(ref b) => {
                    serde_json::from_slice::<outgoing::Message>(b)
                }
                _ => {
                    continue;
                }
            };
            if let Ok(message) = parse_result {
                log::debug!("Send message from websocket to controller: {message:?}");
                to_controller_tx
                    .send(message)
                    .await
                    .expect("unable to send data from websocket to controller");
            }
        }
    }
}
