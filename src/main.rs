use anyhow::Result;
use futures::future::join_all;
use futures::StreamExt;
use gst::glib;
use settings::Settings;
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

fn main() -> Result<()> {
    env_logger::init();
    gst::init()?;

    let main_loop = glib::MainLoop::new(None, false);

    let main_loop_clone = main_loop.clone();
    let _recorder = std::thread::spawn(move || {
        let res = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to start tokio async runtime")
            .block_on(main2());

        if let Err(e) = res {
            eprintln!("Exit on failure: {:?}", e);
            std::process::exit(-1);
        }
        main_loop_clone.quit();
    });

    main_loop.run();

    Ok(())
}

async fn main2() -> Result<()> {
    let settings = Settings::load("config.toml")?;

    let recorder_context = Recorder::create(settings).await?;

    let mut tasks = vec![];

    // TODO react to SIGTERM
    loop {
        match rmq::connect_rabbitmq(&recorder_context.settings.rabbitmq).await {
            Ok(mut consumer) => {
                while let Some(delivery) = consumer.next().await {
                    match delivery {
                        Ok(ref delivery) => {
                            let start_command = rmq::handle_delivery(delivery).await?;
                            let task = recorder_context.spawn_session(start_command)?;

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
                log::error!("RMQ connect error: {:?}", e);
                tasks.retain(|task| !task.is_finished());
                if tasks.is_empty() {
                    log::info!("Exiting after error as all tasks are finished");
                    break;
                }
                log::info!("Retry RMQ connect in {:?}", RECONNECT_INTERVAL);
                sleep(RECONNECT_INTERVAL).await;
            }
        }
    }

    join_all(tasks).await;

    Ok(())
}
