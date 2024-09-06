// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

#![allow(clippy::module_name_repetitions)]

use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use futures::{future::join_all, StreamExt};
use gst::glib;
use http::HttpClient;
use log::warn;
use settings::{HardwareAcceleration, HardwareAccelerationIntel, Settings};
use system_info::{
    cpu::cpu_usage_poll, gpu_intel::gpu_intel_usage_poll, is_new_recording_feasible,
};
use tokio::{
    select,
    signal::{
        ctrl_c,
        unix::{signal, SignalKind},
    },
    sync::watch::{self, Receiver},
    task::JoinHandle,
    time::{sleep, Duration},
};

mod http;
mod recorder;
mod rmq;
mod settings;
mod signaling;
mod system_info;

use crate::recorder::Recorder;

const RECONNECT_INTERVAL: Duration = Duration::from_millis(3_000); //ms
const DOT_OUTPUT_PATH: &str = "./pipelines";

fn check_for_ffmpeg() -> Result<()> {
    _ = std::process::Command::new("ffmpeg")
        .args(["--help"])
        .output()?;

    Ok(())
}

fn check_plugins() -> Result<()> {
    if check_for_ffmpeg().is_err() {
        warn!("ffmpeg is not present on the system. Some features may not work.");
    }

    gstrsinter::plugin_register_static()?;

    let registry = gst::Registry::get();

    let required = [
        "audiomixer",
        "audiotestsrc",
        "autodetect",
        "compositor",
        "debug",
        "dtls",
        "pango",
        "rsinter",
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
        anyhow::bail!("Failed to load GStreamer plugins [{}], try to start the application with 'GST_DEBUG=1' if the plugins are installed correctly.", failed_plugins.join(", "));
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
        anyhow::bail!("The intel_gpu_top command is not installed, this is mandataory for hardware accelaration, please install it or remove hardware acceleration.");
    }

    Ok(())
}

fn main_loop() -> Result<()> {
    env_logger::init();

    if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
        warn!("Using default dot path. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output.");
        std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_OUTPUT_PATH);
    };

    gst::init()?;
    check_plugins()?;

    // Run a MainLoop on a separate thread so gstreamer bus watches work
    let main_loop = glib::MainLoop::new(None, false);
    std::thread::spawn({
        let main_loop = main_loop.clone();

        move || {
            main_loop.run();
        }
    });

    let (shutdown_tx, shutdown_rx) = watch::channel::<bool>(false);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start tokio async runtime")?;

    runtime.spawn(async move {
        let mut sig_term = signal(SignalKind::terminate()).expect("can not setup SIGTERM handler");
        select! {
            _ = ctrl_c() => { log::info!("received Ctrl-C"); }
            _ = sig_term.recv() => { log::info!("received SIGTERM"); }
        }
        shutdown_tx
            .send(true)
            .expect("failed to send shutdown signal");
    });

    if let Err(e) = runtime.block_on(main2(shutdown_rx)) {
        eprintln!("Exit on failure: {e:?}");
        std::process::exit(-1);
    }

    main_loop.quit();

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    main_loop().expect("failed to run main loop");
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

            let res = main_loop();

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

async fn rmq_session(
    recorder_context: &Recorder,
    tasks: &mut Vec<JoinHandle<Result<()>>>,
) -> Result<()> {
    let rabbitmq_settings = &recorder_context.settings.rabbitmq;
    let channel = rmq::open_rabbitmq_connection(rabbitmq_settings).await?;

    match rmq::create_rmq_queue_and_consume(channel, rabbitmq_settings).await {
        Ok(mut consumer) => {
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(ref delivery) => {
                        if !is_new_recording_feasible() {
                            if let Err(e) = delivery
                                .reject(lapin::options::BasicRejectOptions { requeue: true })
                                .await
                            {
                                log::error!("Failed to reject Deliver: {e:?}");
                            }

                            continue;
                        }

                        let start_command = rmq::handle_delivery(delivery).await?;
                        let task = recorder_context
                            .spawn_session(start_command)
                            .await
                            .map_err(|e| {
                                log::error!("Recording session failed: {:?}", e);
                                e
                            })?;

                        tasks.push(task);
                    }
                    Err(e) => {
                        log::error!("RabbitMQ consumer returned error: {:?}", e);
                        break;
                    }
                }
                tasks.retain(|task| !task.is_finished());
            }
        }
        Err(e) => {
            log::error!(
                "RMQ connect error: {:?} (reconnecting in {:?})",
                e,
                RECONNECT_INTERVAL
            );
            sleep(RECONNECT_INTERVAL).await;
        }
    }
    Ok::<(), anyhow::Error>(())
}

async fn main2(mut shutdown_rx: Receiver<bool>) -> Result<()> {
    let settings = Settings::load("config.toml").context("Failed to read config")?;

    let http_client = HttpClient::discover(&settings.auth)
        .await
        .context("OIDC discovery failed")?;
    let recorder_context = Recorder::new(settings, http_client, shutdown_rx.clone());
    let mut tasks: Vec<JoinHandle<Result<()>>> = vec![];
    let mut cutoff = settings::default_max_load();
    if let Some(recorder_info) = &recorder_context.settings.recorder {
        cutoff = recorder_info.max_load;
    }

    let mut usage_handle = if let Some(hardware_acceleration) = recorder_context
        .settings
        .recorder
        .clone()
        .and_then(|s| s.hardware_acceleration)
    {
        log::info!("Hardware Acceleration enabled, using the GPU for encoding");
        match hardware_acceleration {
            HardwareAcceleration::Intel(HardwareAccelerationIntel { device }) => {
                check_intel_gpu_top_command()?;
                tokio::task::spawn_blocking(move || gpu_intel_usage_poll(cutoff, device))
            }
        }
    } else {
        log::info!("Hardware Acceleration disabled, this can cause high cpu load");
        tokio::task::spawn_blocking(move || cpu_usage_poll(cutoff))
    };

    while !*shutdown_rx.borrow() {
        select! {
            result = &mut usage_handle => {
                result??;
            }
            result = shutdown_rx.changed() => {
                result?;
            }
            _ = rmq_session(&recorder_context, &mut tasks) => {}
        }
    }
    tasks.retain(|task| !task.is_finished());

    if !tasks.is_empty() {
        log::info!("waiting for remaining {} tasks to finish", tasks.len());
        join_all(tasks).await;
    }

    Ok(())
}
