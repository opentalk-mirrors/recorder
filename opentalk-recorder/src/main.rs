// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

#![allow(clippy::module_name_repetitions)]

use std::{
    collections::HashMap,
    net::IpAddr,
    process::{exit, Command, Stdio},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::Router;
use clap::Parser;
use futures::future::join_all;
use gst::glib;
use log::warn;
use opentalk_client::{config::ClientConfig, types::api::v1::error::ApiError, OpenTalkClient};
use opentalk_recorder_web_api::v1::{self, RecorderBackend, RecordingAction};
use opentalk_service_auth::service::ApiKeyAuthorization;
use opentalk_types_api_internal::recording::RecordingTarget;
use service_probe::{set_service_state, start_probe, ServiceState};
use service_probe_client::is_ready;
use settings::{HardwareAcceleration, HardwareAccelerationIntel, MonitoringSettings, Settings};
use system_info::{cpu::cpu_usage_poll, gpu_intel::gpu_intel_usage_poll};
use tokio::{
    net::TcpListener,
    select,
    signal::{
        ctrl_c,
        unix::{signal, SignalKind},
    },
    sync::broadcast,
    task::JoinHandle,
};

use crate::cli::{print_info, Commands};

mod cli;
mod recorder;
mod settings;
mod system_info;

use crate::{cli::Args, recorder::Recorder, system_info::is_new_recording_feasible};

#[derive(Clone)]
pub struct AppState {
    pub(crate) recorder_context: Arc<Recorder>,
    pub(crate) tasks: Arc<Mutex<HashMap<RecordingTarget, JoinHandle<Result<()>>>>>,
}

#[async_trait]
impl RecorderBackend for AppState {
    async fn init(&self, recording: RecordingTarget) -> Result<RecordingAction, ApiError> {
        if !is_new_recording_feasible() {
            return Err(ApiError::service_unavailable()
                .with_code("out_of_resources")
                .with_message("No compute resources available"));
        }

        let recorder_context = self.recorder_context.clone();

        if self
            .tasks
            .lock()
            .expect("Failed to acquire task lock")
            .get(&recording)
            .is_some_and(|handle| !handle.is_finished())
        {
            return Ok(RecordingAction::AlreadyRunning);
        }

        let session = Box::pin(recorder_context.clone().spawn_session(recording)).await;
        match session {
            Ok(task) => {
                self.tasks
                    .lock()
                    .expect("Failed to acquire task lock")
                    .insert(recording, task);
                Ok(RecordingAction::Created)
            }
            Err(err) => {
                log::error!("Failed to start recording session: {err:?}");

                Err(ApiError::internal().with_message("Failed to start recording session"))
            }
        }
    }
}

const DOT_OUTPUT_PATH: &str = "./pipelines";

fn check_plugins() -> Result<()> {
    let registry = gst::Registry::get();

    let required = [
        "audiomixer",
        "audiotestsrc",
        "autodetect",
        "compositor",
        "debug",
        "dtls",
        "pango",
        "png",
        "rtp",
        "srtp",
        "udp",
        "vaapi",
        "videotestsrc",
        "vpx",
        "webrtc",
    ];

    let failed_plugins: Vec<_> = required
        .into_iter()
        .filter(|plug| registry.find_plugin(plug).is_none())
        .collect();

    if !failed_plugins.is_empty() {
        anyhow::bail!(
            "Failed to load GStreamer plugins [{}], try to start the application with 'GST_DEBUG=1' if the plugins are installed correctly.",
            failed_plugins.join(", ")
        );
    }

    Ok(())
}

fn check_intel_gpu_top_command() -> Result<()> {
    let status = Command::new("intel_gpu_top")
        .arg("-h")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map(|status| status.success());

    if status.is_err() || status.ok() == Some(false) {
        anyhow::bail!(
            "The intel_gpu_top command is not installed, this is mandataory for hardware accelaration, please install it or remove hardware acceleration."
        );
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    main_loop().await.expect("failed to run main loop");
    Ok(())
}

#[cfg(target_os = "macos")]
fn main() {
    // This code sequence is adapted from `gstreamer-rs` (https://github.com/sdroege/gstreamer-rs) under the MIT License.
    // Original source: https://github.com/sdroege/gstreamer-rs/blob/main/examples/src/examples-common.rs#L15
    // Copyright 2024 Sebastian Dröge
    use std::{
        ffi::c_void,
        sync::mpsc::{channel, Sender},
        thread,
    };

    use cocoa::{
        appkit::{NSApplication, NSWindow},
        base::id,
        delegate,
    };
    use objc::{
        class, msg_send,
        runtime::{Object, Sel},
        sel, sel_impl,
    };

    unsafe {
        extern "C" fn on_finish_launching(this: &Object, _cmd: Sel, _notification: id) {
            let send = unsafe {
                let send_pointer = *this.get_ivar::<*const c_void>("send");
                let boxed = Box::from_raw(send_pointer as *mut Sender<()>);
                *boxed
            };
            send.send(()).expect("failed to send to main thread");
        }

        let app = cocoa::appkit::NSApp();
        let (send, recv) = channel::<()>();

        let delegate = delegate!("AppDelegate", {
            app: id = app,
            send: *const c_void = Box::into_raw(Box::new(send)) as *const c_void,
            (applicationDidFinishLaunching:) => on_finish_launching as extern fn(&Object, Sel, id)
        });
        app.setDelegate_(delegate);

        let t = thread::spawn(move || {
            // Wait for the NSApp to launch to avoid possibly calling stop_() too early
            recv.recv().expect("failed to receive from main thread");

            let res = main_loop().await;

            let app = cocoa::appkit::NSApp();
            app.stop_(cocoa::base::nil);

            // Stopping the event loop requires an actual event
            let event = cocoa::appkit::NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2_(
                cocoa::base::nil,
                cocoa::appkit::NSEventType::NSApplicationDefined,
                cocoa::foundation::NSPoint { x: 0.0, y: 0.0 },
                cocoa::appkit::NSEventModifierFlags::empty(),
                0.0,
                0,
                cocoa::base::nil,
                cocoa::appkit::NSEventSubtype::NSApplicationActivatedEventType,
                0,
                0,
            );
            app.postEvent_atStart_(event, cocoa::base::YES);

            res
        });

        app.run();

        let _ = t.join().expect("failed to join thread");
    }
}

async fn main_loop() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    if args.info.should_print() {
        print_info(&args.info);
        return Ok(());
    }

    let settings = Arc::new(settings::Settings::load(args.config.as_ref())?);
    if let Some(Commands::Health { endpoint }) = args.command {
        let Some(monitoring_endpoint) = endpoint.or_else(|| {
            settings.monitoring.as_ref().map(|monitoring_settings| {
                format!(
                    "http://{}:{}",
                    monitoring_settings.addr, monitoring_settings.port
                )
                .parse()
                .expect("valid endpoint can be built from monitoring settings")
            })
        }) else {
            log::warn!("Monitoring not configured and no url endpoint parameter given");
            exit(1);
        };
        return match is_ready(&monitoring_endpoint).await {
            Ok(true) => {
                log::info!("READY");
                Ok(())
            }
            Ok(false) => {
                log::info!("Not Ready");
                exit(1)
            }
            Err(err) => {
                log::error!("Err: {err}");
                exit(-1)
            }
        };
    }
    if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
        warn!(
            "Using default dot path. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output."
        );
        unsafe {
            std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_OUTPUT_PATH);
        }
    }

    let _ = tokio::task::spawn_blocking(move || -> Result<()> {
            if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
                warn!(
                    "Using default dot path. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output."
                );
                unsafe {
                    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_OUTPUT_PATH);
                }
            }

            gst::init()?;
            check_plugins()?;

            // Run a MainLoop on a separate thread so gstreamer bus watches work
            let gstreamer_main_loop = glib::MainLoop::new(None, false);
            std::thread::spawn({
                let main_loop = gstreamer_main_loop.clone();

                move || {
                    main_loop.run();
                }
            });

            let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("failed to start tokio async runtime")?;


            runtime.spawn(async move {
                let mut sig_term =
                    signal(SignalKind::terminate()).expect("can not setup SIGTERM handler");
                select! {
                    _ = ctrl_c() => { log::info!("received Ctrl-C"); }
                    _ = sig_term.recv() => { log::info!("received SIGTERM"); }
                }
                shutdown_tx
                    .send(())
                    .expect("failed to send shutdown signal");
            });

            if let Err(e) = runtime.block_on(run_recorder(shutdown_rx, settings)) {
                eprintln!("Exit on failure: {e:?}");
                std::process::exit(-1);
            }

            log::debug!("Send quit to main_loop");
            gstreamer_main_loop.quit();
            Ok(())
        }).await?;
    Ok(())
}
async fn run_recorder(
    mut shutdown_rx: broadcast::Receiver<()>,
    settings: Arc<Settings>,
) -> Result<()> {
    let client = OpenTalkClient::create(ClientConfig {
        auth: opentalk_client::AuthConfig::ClientCredentials(
            opentalk_client::config::ClientCredentialsAuthConfig {
                issuer: settings.auth.issuer.clone(),
                client_id: settings.auth.client_id.clone(),
                client_secret: settings.auth.client_secret.clone(),
            },
        ),
        controller: opentalk_client::ControllerConfig {
            domain: settings.controller.domain.clone(),
            insecure: settings.controller.insecure,
        },
    })
    .await?;

    let recorder_context = Recorder::new(settings.clone(), client, shutdown_rx.resubscribe());
    let mut tasks: Vec<JoinHandle<Result<()>>> = vec![];

    let recorder = AppState {
        tasks: Arc::new(Mutex::new(HashMap::new())),
        recorder_context: Arc::new(recorder_context.clone()),
    };

    if let Some(MonitoringSettings { port, addr }) = settings.monitoring {
        start_probe(addr, port, ServiceState::Up).await?;
    }

    let auth_middleware = settings
        .http
        .api_keys
        .auth_middleware()
        .context("Invalid API key configuration")?;

    select! {
        _ =  shutdown_rx.recv() => {
            log::info!("Received shutdown, shutdown all remaining tasks");
        }
        _ = run_usage_polling(&recorder_context) => {
            log::debug!("Usage polling failed, shutdown all remaining tasks");
        }
        result = run_axum_server(settings.http.addr, settings.http.port, recorder.clone(), auth_middleware) => { result?; }
    }
    tasks.retain(|task| !task.is_finished());

    if !tasks.is_empty() {
        log::info!("waiting for remaining {} tasks to finish", tasks.len());
        join_all(tasks).await;
    }
    log::info!("All tasks are finished");

    Ok(())
}

async fn run_axum_server(
    address: IpAddr,
    port: u16,
    recorder: AppState,
    auth_middleware: ApiKeyAuthorization,
) -> Result<()> {
    let app = Router::new()
        .merge(v1::routes().layer(auth_middleware))
        .with_state(recorder);

    let listener = TcpListener::bind((address, port)).await?;

    // Server up and running, ready to process requests
    set_service_state(ServiceState::Ready);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_usage_polling(recorder_context: &Recorder) -> Result<(), broadcast::error::RecvError> {
    let hardware_acceleration = recorder_context
        .settings
        .recorder
        .clone()
        .and_then(|s| s.hardware_acceleration);
    let mut cutoff = settings::default_max_load();
    if let Some(recorder_info) = &recorder_context.settings.recorder {
        cutoff = recorder_info.max_load;
    }

    let run_blocking = move || -> Result<()> {
        if let Some(HardwareAcceleration::Intel(HardwareAccelerationIntel { device })) =
            hardware_acceleration
        {
            log::info!("Hardware Acceleration enabled, using the GPU for encoding");
            check_intel_gpu_top_command()?;
            gpu_intel_usage_poll(cutoff, device)?;
        } else {
            log::info!("Hardware Acceleration disabled, this can cause high cpu load");
            cpu_usage_poll(cutoff)?;
        }

        Ok(())
    };

    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);
    std::thread::spawn(move || {
        if let Err(err) = run_blocking() {
            log::error!("Usage polling failed, received: {err}");
            shutdown_tx
                .send(())
                .expect("Unable to send shutdown based on usage polling error");
        }
    });

    shutdown_rx.recv().await
}
