use futures::future::join_all;
use libs::{config, error::Error};
use log::error;

mod libs;
extern crate clap;

#[tokio::main]
async fn main() {
    setup_logger();
    match start().await {
        Ok(_) => {}
        Err(err) => error!("{}", err),
    }
}

fn setup_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{:5}]{}",
                chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(if cfg!(test) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .level_for(env!("CARGO_PKG_NAME"), log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

async fn start() -> Result<(), Error> {
    let handlers = config::configuration()?
        .create_updaters()
        .into_iter()
        .map(|mut updater| {
            tokio::spawn(async move {
                updater.start().await;
            })
        });

    join_all(handlers).await;

    Ok(())
}
