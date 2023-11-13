// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    time::Duration,
};

use gst::{glib, traits::ElementExt};
use opentalk_recorder::signaling::{incoming, outgoing};
use tokio::{
    sync::{mpsc, watch, Mutex},
    time::sleep,
};

use crate::common::{
    controller::MockController, logger::PanicLogger, recorder::start_recorder,
    websocket_server::start_websocket_server,
};

mod controller;
mod logger;
mod recorder;
mod webrtc;
mod websocket_server;

#[allow(unused_imports, dead_code)]
pub(crate) mod prelude;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum Event {
    /// Simulate a user join with a specific index
    JoinUser(usize),

    /// Join n users with with audio, video, screen share
    JoinUsers(usize, bool, bool, bool),

    /// Simulate a user leave with a specific index
    LeftUser(usize),

    /// Wait the duration of time until the next event will be fired
    Sleep(Duration),

    /// Set the Speacer focus for the specific user index
    SpeakerFocusSet(usize),
    /// Unsets the speaker focus
    SpeakerFocusUnset,

    /// Starts the recording
    StartRecording,

    /// Stops the recording
    StopRecording,

    /// Update the consent for the user with the given index
    UpdateConsent(usize, bool),

    /// Update the consent for n users
    UpdateConsents(usize, bool),

    /// Update the media with audio, video and screen share for the user with the given index
    UpdateMedia(usize, bool, bool, bool),
}

pub(crate) struct User {
    participant: incoming::Participant,
    webrtc_pipeline: Option<gst::Pipeline>,
}

pub(crate) struct EventRunner {
    users: Arc<Mutex<BTreeMap<usize, User>>>,
    mock_controller: MockController,
}

impl EventRunner {
    pub(crate) async fn run(events: &[Event]) {
        let result = tokio::task::spawn_blocking({
            let events = events.to_owned();

            move || {
                tokio::runtime::Handle::current().block_on(async move {
                    Self::run_events(events).await;
                })
            }
        })
        .await;
        result.expect("Event handler thread crashed");
    }

    async fn run_events(events: Vec<Event>) {
        println!("Initialize env_logger");
        let error_occurred = Arc::new(AtomicBool::new(false));
        PanicLogger::init(error_occurred.clone());

        log::info!("Initialize gstreamer");
        gst::init().expect("unable to init gst");

        log::info!("Start gstreamers MainLoop");
        let main_loop = glib::MainLoop::new(None, false);
        std::thread::spawn({
            let main_loop = main_loop.clone();

            move || {
                main_loop.run();
            }
        });

        let (shutdown_tx, shutdown_rx) = watch::channel::<bool>(false);
        let (to_recorder_tx, to_recorder_rx) = mpsc::channel::<incoming::Message>(20);
        let (to_controller_tx, to_controller_rx) = mpsc::channel::<outgoing::Message>(20);

        let users = Arc::new(Mutex::new(BTreeMap::<usize, User>::new()));
        let websocket_addr = start_websocket_server(to_recorder_rx, to_controller_tx).await;
        let mock_controller =
            MockController::run(users.clone(), to_recorder_tx.clone(), to_controller_rx);

        let mut event_runner = Self {
            users,
            mock_controller,
        };

        log::info!("Wait 1 second to ensure that gstreamer has started properly");
        event_runner.sleep(Duration::from_secs(1)).await;

        log::debug!("Server started, start listening to events...");
        for event in events {
            match event {
                Event::JoinUser(index) => event_runner.join_user(index).await,
                Event::JoinUsers(amount, audio, video, screen) => {
                    event_runner.join_users(amount, audio, video, screen).await
                }
                Event::LeftUser(index) => event_runner.left_user(index).await,
                Event::Sleep(duration) => event_runner.sleep(duration).await,
                Event::SpeakerFocusSet(index) => event_runner.speaker_focus_set(index).await,
                Event::SpeakerFocusUnset => event_runner.speaker_focus_unset().await,
                Event::StartRecording => {
                    log::info!("Start the recorder, everyone should be able to give consent");
                    start_recorder(websocket_addr, shutdown_rx.clone()).await;
                    event_runner.start_recorder().await
                }
                Event::StopRecording => {
                    log::info!("Stop the recording");
                    shutdown_tx
                        .send(true)
                        .expect("unable to send shutdown command for the recorder");
                }
                Event::UpdateConsent(index, consent) => {
                    event_runner.update_consent(index, consent).await
                }
                Event::UpdateConsents(amount, consent) => {
                    event_runner.update_consents(amount, consent).await
                }
                Event::UpdateMedia(index, audio, video, screen) => {
                    event_runner.update_media(index, audio, video, screen).await
                }
            }
        }

        log::debug!("Send stop event to the recorder");
        shutdown_tx
            .send(true)
            .expect("unable to send shutdown request");

        log::debug!("Stop the MainLoop for gstreamer");
        main_loop.quit();

        assert!(
            !error_occurred.load(atomic::Ordering::Relaxed),
            "GStreamer error was logged"
        );
    }

    async fn start_recorder(&mut self) {
        log::info!("StartRecorder event received");
        self.mock_controller.send_join_success().await;
    }

    async fn join_user(&mut self, index: usize) {
        log::info!("JoinUser event received, join user, index: '{index}'");
        let participant = self.mock_controller.send_joined(index).await;
        let mut users = self.users.lock().await;
        users.insert(
            index,
            User {
                participant,
                webrtc_pipeline: None,
            },
        );
    }

    async fn join_users(&mut self, amount: usize, audio: bool, video: bool, screen: bool) {
        log::info!("JoinUsers event received, join '{amount}' users with, audio: '{audio}', video: '{video}', screen: '{screen}'");
        let mut users = self.users.lock().await;
        let highest_index = users.keys().max().map(|index| index + 1).unwrap_or(0);
        for index in highest_index..(highest_index + amount) {
            log::info!("-> join user, index: '{index}'");
            let mut participant = self.mock_controller.send_joined(index).await;
            log::info!("-> update media, index: '{index}', audio: '{audio}', video: '{video}', screen: '{screen}'");
            self.mock_controller
                .send_update_media(&mut participant, audio, video, screen)
                .await;
            users.insert(
                index,
                User {
                    participant,
                    webrtc_pipeline: None,
                },
            );
        }
    }

    async fn left_user(&mut self, index: usize) {
        log::info!("LeftUserevent received, left user, index: '{index}'");
        let mut users = self.users.lock().await;
        if let Some(user) = users.remove(&index) {
            self.mock_controller.send_left(&user.participant).await;
            if let Some(webrtc_pipeline) = user.webrtc_pipeline {
                webrtc_pipeline
                    .set_state(gst::State::Null)
                    .expect("unable to set state of webrtc_pipeline to null");
            }
        }
    }
    async fn sleep(&self, duration: Duration) {
        log::info!("Sleep event received, wait {}", duration.as_secs());
        sleep(duration).await;
    }

    async fn speaker_focus_set(&mut self, index: usize) {
        log::info!("SpeakerFocusSet event received, index: '{index}'");

        let mut users = self.users.lock().await;
        let user = users
            .get_mut(&index)
            .expect("unable to find participant with the given index");
        self.mock_controller
            .send_update_focus(Some(&user.participant))
            .await;
    }

    async fn speaker_focus_unset(&mut self) {
        log::info!("SpeakerFocusUnset event received");
        self.mock_controller.send_update_focus(None).await;
    }

    async fn update_consent(&mut self, index: usize, consent: bool) {
        log::info!("UpdateConsent event received, update consent for the user, index: '{index}', consent: '{consent}'");

        let mut users = self.users.lock().await;
        let user = users
            .get_mut(&index)
            .expect("unable to find participant with the given index");
        self.mock_controller
            .send_update_consent(&mut user.participant, consent)
            .await;
    }

    async fn update_consents(&mut self, amount: usize, consent: bool) {
        log::info!("UpdateConsents event received for '{amount}' users with consent: {consent}");
        let mut users = self.users.lock().await;
        for (index, user) in users.iter_mut().take(amount) {
            log::info!("-> update consent for index: '{index}', consent: {consent}",);
            self.mock_controller
                .send_update_consent(&mut user.participant, consent)
                .await;
        }
    }

    async fn update_media(&mut self, index: usize, audio: bool, video: bool, screen: bool) {
        log::info!("UpdateMedia event received, update media for the user, index: '{index}', audio: '{audio}', video: '{video}', screen: '{screen}'");

        let mut users = self.users.lock().await;
        let user = users
            .get_mut(&index)
            .expect("unable to find user with the given index");
        self.mock_controller
            .send_update_media(&mut user.participant, video, audio, screen)
            .await;
    }
}
