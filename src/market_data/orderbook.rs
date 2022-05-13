use std::{cmp::Reverse, collections::HashMap};

use float_ord::FloatOrd;
use tokio::sync::{broadcast, mpsc};

use crate::{msg, server, shutdown, SETTINGS};

pub struct Orderbook {
    pub levels_rx: mpsc::Receiver<msg::Levels>,
    pub summary_tx: broadcast::Sender<server::orderbook::Summary>,
    pub summary_rx: broadcast::Receiver<server::orderbook::Summary>,
    pub shutdown_rx: shutdown::Receiver,
}

impl Orderbook {
    pub async fn aggregate(&mut self) {
        let mut map = LevelMap::new();

        loop {
            tokio::select! {
                msg = self.levels_rx.recv() => {
                    match msg {
                        Some(levels) => {
                            let summary = map.update(levels);

                            if let Err(err) = self.summary_tx.send(summary) {
                                eprintln!("Unable to send summary: {}", err);
                                break;
                            }
                        }
                        None => break,
                    }
                },
                _ = self.shutdown_rx.recv() => break,
            }
        }
        println!("Exiting orderbook...");
    }
}

struct LevelMap {
    exchange_map: HashMap<&'static str, msg::Levels>,
}

impl LevelMap {
    fn new() -> LevelMap {
        LevelMap {
            exchange_map: HashMap::new(),
        }
    }

    fn update(&mut self, levels: msg::Levels) -> server::orderbook::Summary {
        self.exchange_map.insert(levels.exchange, levels);

        let mut bids: Vec<_> = self
            .exchange_map
            .values()
            .flat_map(|levels| &levels.bids)
            .collect();

        let mut asks: Vec<_> = self
            .exchange_map
            .values()
            .flat_map(|levels| &levels.asks)
            .collect();

        bids.sort_unstable_by_key(|bid| Reverse(FloatOrd(bid.price)));
        asks.sort_unstable_by_key(|ask| FloatOrd(ask.price));

        let bids: Vec<server::orderbook::Level> = bids
            .into_iter()
            .take(SETTINGS.app.summary_size)
            .map(|s| s.into())
            .collect();

        let asks: Vec<server::orderbook::Level> = asks
            .into_iter()
            .take(SETTINGS.app.summary_size)
            .map(|s| s.into())
            .collect();

        let spread = if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
            best_ask.price - best_bid.price
        } else {
            0.0
        };

        server::orderbook::Summary { spread, bids, asks }
    }
}
