// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use compositor::{FakeSink, Mixer, TestSink};
use openidconnect::{
    core::{CoreClient, CoreJsonWebKeySet},
    AccessToken, AuthUrl, ClientId, ClientSecret, IssuerUrl,
};
use opentalk_recorder::{
    http::HttpClient,
    recorder::{Recorder, RecordingSession, MAX_VISIBLES},
    settings::{AuthSettings, ControllerSettings, RabbitMqSettings, Settings},
    signaling::Signaling,
};
use tempfile::TempDir;
use tokio::{
    sync::{mpsc, watch, RwLock},
    task::JoinHandle,
};
use tt::{connect_async, tungstenite::client::IntoClientRequest};

pub(crate) async fn start_recorder(
    controller_addr: SocketAddr,
    websocket_addr: SocketAddr,
    shutdown_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    log::info!("Start recorder...");

    let issuer = IssuerUrl::new("http://127.0.0.1".to_string()).unwrap();
    let client_id = ClientId::new("NOT_USED_IN_TESTS".to_string());
    let auth = AuthSettings {
        issuer: issuer.clone(),
        client_id: client_id.clone(),
        client_secret: ClientSecret::new("NOT_USED_IN_TESTS".to_string()),
    };
    let controller = ControllerSettings {
        domain: controller_addr.to_string(),
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
    let signaling = Signaling::new(None, connection);
    let (candidate_sender, candidate_receiver) = mpsc::channel(12);
    let temp_dir = TempDir::new().expect("unable to create temp dir");

    let mut mixer = Mixer::create(
        compositor::Size::FHD,
        compositor::layout::Speaker::default(),
        MAX_VISIBLES,
        true,
        &Default::default(),
    )
    .expect("unable to create talk");

    mixer
        .link_sink("test_sink", TestSink::create("TestSink", true).unwrap())
        .unwrap();
    mixer
        .link_sink(
            "fake_sink_with_video",
            FakeSink::create("FakeSink with video", true).unwrap(),
        )
        .unwrap();
    mixer
        .link_sink(
            "fake_sink_without_video",
            FakeSink::create("FakeSink without video", false).unwrap(),
        )
        .unwrap();

    let mut recording_session = RecordingSession::new(
        Arc::new(recorder),
        signaling,
        HashMap::new(),
        "TESTROOM".to_string(),
        temp_dir,
        mixer,
        BTreeMap::new(),
        candidate_receiver,
        candidate_sender,
        false,
    );

    tokio::spawn(async move {
        recording_session
            .run()
            .await
            .expect("unable to run recording session");
    })
}
