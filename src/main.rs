use anyhow::Result;
use futures::future::join_all;
use futures::StreamExt;
use gst::glib;
use http::HttpClient;
use log::warn;
use settings::Settings;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

mod http;
mod recorder;
mod rmq;
mod settings;
mod signaling;

use crate::recorder::Recorder;

//#[cfg(test)]
//mod tests;

const RECONNECT_INTERVAL: Duration = Duration::from_millis(3_000); //ms
const DOT_OUTPUT_PATH: &str = "./pipelines";

fn main() -> Result<()> {
    env_logger::init();

    if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
        warn!("Using default dot path. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output.");
        std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", DOT_OUTPUT_PATH);
    };

    gst::init()?;

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
        .expect("Failed to start tokio async runtime");

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
        eprintln!("Exit on failure: {:?}", e);
        std::process::exit(-1);
    }

    main_loop.quit();

    Ok(())
}

async fn rmq_session(
    recorder_context: &Recorder,
    tasks: &mut Vec<JoinHandle<Result<()>>>,
) -> Result<()> {
    match rmq::connect_rabbitmq(&recorder_context.settings.rabbitmq).await {
        Ok(mut consumer) => {
            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(ref delivery) => {
                        let start_command = rmq::handle_delivery(delivery).await?;
                        let task = recorder_context
                            .spawn_session(start_command)
                            .await
                            .map_err(|e| {
                                log::error!("Recording session failed: {}", e);
                                e
                            })?;

                        tasks.push(task);
                    }
                    Err(e) => {
                        log::error!("RabbitMQ consumer returned error: {}", e);
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
    let settings = Settings::load("config.toml")?;

    let http_client = HttpClient::discover(&settings.auth).await?;
    let recorder_context = Recorder::new(settings, http_client, shutdown_rx.clone());
    let mut tasks: Vec<JoinHandle<Result<()>>> = vec![];

    while !*shutdown_rx.borrow() {
        select! {
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
