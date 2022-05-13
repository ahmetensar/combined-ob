#[macro_use]
extern crate lazy_static;

mod exchange;
mod market_data;
mod msg;
mod server;
mod settings;
mod shutdown;

use tokio::sync::{broadcast, mpsc};

lazy_static! {
    static ref SETTINGS: settings::Settings =
        settings::Settings::new().expect("settings can't be loaded");
}

#[tokio::main]
async fn main() {
    let shutdown_tx = shutdown::Sender::new();

    process_market_data(&shutdown_tx);

    match tokio::signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // we also shut down in case of error
        }
    }

    shutdown_tx.send().await;
}

fn process_market_data(shutdown_tx: &shutdown::Sender) {
    let (levels_tx, levels_rx) = mpsc::channel::<msg::Levels>(SETTINGS.app.channel_capacity);
    let (summary_tx, summary_rx) = broadcast::channel(SETTINGS.app.channel_capacity);

    let levels_tx_clone = levels_tx.clone();
    let shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async {
        let mut binance = exchange::Binance {
            levels_tx: levels_tx_clone,
            shutdown_rx,
        };
        binance.connect().await
    });

    let shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async {
        let mut bitstamp = exchange::Bitstamp {
            levels_tx,
            shutdown_rx,
        };
        bitstamp.connect().await
    });

    let summary_tx_clone = summary_tx.clone();
    let shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async {
        let mut orderbook = market_data::Orderbook {
            levels_rx,
            summary_tx: summary_tx_clone,
            summary_rx,
            shutdown_rx,
        };
        orderbook.aggregate().await
    });

    let shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async {
        let mut server = server::Server { shutdown_rx };
        server.serve(summary_tx).await
    });
}
