// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use compositor::{FakeSink, Talk, TestSink};
use openidconnect::core::CoreJsonWebKeySet;
use openidconnect::{core::CoreClient, AccessToken, AuthUrl, ClientId, ClientSecret, IssuerUrl};
use opentalk_recorder::{
    http::HttpClient,
    recorder::{Recorder, MAX_VISIBLES},
    settings::{AuthSettings, ControllerSettings, RabbitMqSettings, Settings},
};
use opentalk_recorder::{recorder::RecordingSession, signaling::Signaling};
use tempfile::TempDir;
use tokio::sync::{mpsc, watch, RwLock};
use tt::{connect_async, tungstenite::client::IntoClientRequest};

pub(crate) async fn start_recorder(websocket_addr: SocketAddr, shutdown_rx: watch::Receiver<bool>) {
    log::info!("Start recorder...");

    let issuer = IssuerUrl::new("http://127.0.0.1".to_string()).unwrap();
    let client_id = ClientId::new("NOT_USED_IN_TESTS".to_string());
    let auth = AuthSettings {
        issuer: issuer.clone(),
        client_id: client_id.clone(),
        client_secret: ClientSecret::new("NOT_USED_IN_TESTS".to_string()),
    };
    let controller = ControllerSettings {
        domain: "127.0.0.1".to_string(),
        insecure: true,
    };
    let rabbitmq = RabbitMqSettings {
        uri: Default::default(),
        queue: "NOT_USED_IN_TESTS".to_string(),
    };
    let settings = Settings {
        auth,
        controller,
        rabbitmq,
        recorder: None,
    };
    let client = reqwest::Client::new();

    let oidc = CoreClient::new(
        client_id,
        None,
        issuer,
        AuthUrl::new("http://127.0.0.1".to_string()).unwrap(),
        None,
        None,
        CoreJsonWebKeySet::default(),
    );
    let access_token = RwLock::new(AccessToken::new("NOT_USED_IN_TESTS".to_string()));
    let http_client = HttpClient::new(client, oidc, access_token);
    let recorder = Recorder::new(settings, http_client, shutdown_rx);

    let websocket_request = format!("ws://{websocket_addr}")
        .into_client_request()
        .expect("unable to parse url to client request");
    let (connection, _) = connect_async(websocket_request)
        .await
        .expect("Client failed to connect");
    let signaling = Signaling::new(None, HashMap::new(), connection);
    let (candidate_sender, candidate_receiver) = mpsc::channel(12);
    let temp_dir = TempDir::new().expect("unable to create temp dir");

    let mut talk = Talk::new(
        compositor::Size::FHD,
        compositor::layout::Speaker::default(),
        MAX_VISIBLES,
        true,
    )
    .expect("unable to create talk");

    talk.link_sink("test_sink", TestSink::create("TestSink", true).unwrap())
        .unwrap();
    talk.link_sink(
        "fake_sink_with_video",
        FakeSink::create("FakeSink with video", true).unwrap(),
    )
    .unwrap();
    talk.link_sink(
        "fake_sink_without_video",
        FakeSink::create("FakeSink without video", false).unwrap(),
    )
    .unwrap();

    let mut recording_session = RecordingSession::new(
        Arc::new(recorder),
        signaling,
        "TESTROOM".to_string(),
        temp_dir,
        talk,
        candidate_receiver,
        candidate_sender,
        false,
    );

    tokio::spawn(async move {
        recording_session
            .run()
            .await
            .expect("unable to run recording session")
    });
}
