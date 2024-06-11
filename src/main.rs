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

#[cfg(test)]
fn setup_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{:5}]{}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .level_for(env!("CARGO_PKG_NAME"), log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

#[cfg(not(test))]
fn setup_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{:5}]{}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for(env!("CARGO_PKG_NAME"), log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

async fn start() -> Result<(), Error> {
    let configuration = config::configuration()?;
    let updaters = configuration.create_updaters();

    let handlers = updaters.into_iter().map(|mut updater| {
        tokio::spawn(async move {
            updater.start().await;
        })
    });

    join_all(handlers).await;

    Ok(())
}
