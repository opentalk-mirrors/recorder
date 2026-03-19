// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

#![allow(clippy::module_name_repetitions)]

use std::{
    pin::pin,
    process::{exit, Command, Stdio},
    sync::Arc,
};

use anyhow::{Context, Result};
use futures::{future::join_all, StreamExt};
use gst::glib;
use log::warn;
use opentalk_client::{config::ClientConfig, OpenTalkClient};
use service_probe::{set_service_state, start_probe, ServiceState};
use service_probe_client::is_ready;
use settings::{HardwareAcceleration, HardwareAccelerationIntel, MonitoringSettings, Settings};
use system_info::{
    cpu::cpu_usage_poll, gpu_intel::gpu_intel_usage_poll, is_new_recording_feasible,
};
use tokio::{
    select,
    signal::{
        ctrl_c,
        unix::{signal, SignalKind},
    },
    sync::broadcast,
    task::JoinHandle,
    time::{sleep, Duration},
};

mod cli;
mod recorder;
mod rmq;
mod settings;
mod system_info;

use clap::Parser;

use crate::{
    cli::{print_info, Args, Commands},
    recorder::Recorder,
};
const RECONNECT_INTERVAL: Duration = Duration::from_secs(3);
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

/// `rustls` depend on a `CryptoProvider` being configured.
/// If no provider was explicitly configured, a provider will be derived from
/// the enabled features. Since there are many crates that depend on `rustls`, we
/// don't have complete control over the enabled features.
/// If the configuration via feature is ambiguous `rustls` will panic.
///
/// Here we ensure that these crates are explicitly configured.
fn ensure_crypto_provider() {
    rustls::crypto::CryptoProvider::install_default(rustls::crypto::aws_lc_rs::default_provider())
        .expect("valid default crypto provider expected");
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    ensure_crypto_provider();
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

    if let Some(MonitoringSettings { port, addr }) = settings.monitoring {
        start_probe(addr, port, ServiceState::Up).await?;
    }

    select! {
        _ = shutdown_rx.recv() => {
            log::debug!("Received shutdown, shutdown all remaining tasks");
        }
        _ = run_usage_polling(&recorder_context) => {
            log::debug!("Usage polling failed, shutdown all remaining tasks");
        }
        () = run_rabbitmq_session(&recorder_context, &mut tasks) => {}
    }
    tasks.retain(|task| !task.is_finished());

    if !tasks.is_empty() {
        log::info!("waiting for remaining {} tasks to finish", tasks.len());
        join_all(tasks).await;
    }
    log::info!("All tasks are finished");

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

async fn run_rabbitmq_session(
    recorder_context: &Recorder,
    tasks: &mut Vec<JoinHandle<Result<()>>>,
) {
    loop {
        set_service_state(ServiceState::Up);
        log::debug!("Trying to connect to RabbitMQ");

        let rabbitmq_settings = &recorder_context.settings.rabbitmq;
        let channel = match rmq::open_rabbitmq_connection(rabbitmq_settings).await {
            Ok(channel) => channel,
            Err(err) => {
                log::error!(
                    "Unable to connect to RabbitMQ (reconnecting in {RECONNECT_INTERVAL:?})\n{err:?}"
                );
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        log::trace!("Creating rabbitmq queue");
        let mut consumer = match rmq::create_rmq_queue_and_consume(channel, rabbitmq_settings).await
        {
            Ok(consumer) => consumer,
            Err(err) => {
                log::error!(
                    "Unable to create RabbitMQ queue and consume (reconnecting in {RECONNECT_INTERVAL:?})\n{err:?}"
                );
                sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        log::info!("RabbitMQ connection established successfully");
        set_service_state(ServiceState::Ready);

        while let Some(delivery) = consumer.next().await {
            let delivery = match delivery {
                Ok(delivery) => delivery,
                Err(err) => {
                    log::error!("RabbitMQ consumer returned error\n{err:?}");
                    break;
                }
            };

            log::trace!("Testing for feasibility");
            if !is_new_recording_feasible() {
                if let Err(err) = delivery
                    .reject(lapin::options::BasicRejectOptions { requeue: true })
                    .await
                {
                    log::error!("RabbitMQ failed to reject deliver\n{err:?}");
                    break;
                }
                continue;
            }

            log::trace!("Handling delivery...");
            let start_command = match rmq::handle_delivery(&delivery).await {
                Ok(start_command) => start_command,
                Err(err) => {
                    log::error!("RabbitMQ handle delivery failed\n{err:?}");
                    continue;
                }
            };

            let task = match pin!(recorder_context.spawn_session(start_command)).await {
                Ok(task) => task,
                Err(err) => {
                    log::error!("Recording session failed\n{err:?}");
                    continue;
                }
            };

            tasks.push(task);
            tasks.retain(|task| !task.is_finished());
        }
    }
}
