use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    exchange::util::{read_from_stream, LoopState},
    msg::{Level, Levels},
    shutdown, SETTINGS,
};

type WriteSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

pub struct Binance {
    pub levels_tx: mpsc::Sender<Levels>,
    pub shutdown_rx: shutdown::Receiver,
}

const EXCHANGE: &str = "Binance";
const BINANCE_WSS: &str = "wss://stream.binance.com:9443/ws";

impl Binance {
    pub async fn connect(&mut self) {
        let url = url::Url::parse(BINANCE_WSS).unwrap();
        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Failed to connect to Binance");

        let (mut write, mut read) = ws_stream.split();

        Self::subscribe_to_partial_book_depth(&mut write, 1).await;
        let res = read_from_stream::<Response>(&mut read).await.unwrap();
        assert_eq!(None, res.result);
        assert_eq!(1, res.id);

        loop {
            tokio::select! {
                res = read_from_stream::<PartialBookDepth>(&mut read) => {
                    match res {
                        Ok(data) => {
                            self.process_partial_book_depth(data).await;
                        },
                        Err(state) => {
                            if let LoopState::Break = state {
                                break;
                            }
                        },
                    }
                },
                _ = self.shutdown_rx.recv() => {
                    break;
                }
            };
        }

        Self::unsubscribe_from_partial_book_depth(&mut write, 2).await;

        let _ = write.close();

        println!("Exiting binance...");
    }

    async fn process_partial_book_depth(&self, book: PartialBookDepth) {
        let bids: Vec<_> = book
            .bids
            .into_iter()
            .map(|b| Level {
                exchange: EXCHANGE,
                price: b[0].parse().unwrap(),
                amount: b[1].parse().unwrap(),
            })
            .collect();

        let asks: Vec<_> = book
            .asks
            .into_iter()
            .map(|b| Level {
                exchange: EXCHANGE,
                price: b[0].parse().unwrap(),
                amount: b[1].parse().unwrap(),
            })
            .collect();

        let levels = Levels {
            exchange: EXCHANGE,
            bids,
            asks,
        };

        if let Err(err) = self.levels_tx.send(levels).await {
            eprintln!("Error sending message: {}", err);
        }
    }

    async fn subscribe_to_partial_book_depth(write: &mut WriteSink, id: usize) {
        let request = Request {
            method: String::from("SUBSCRIBE"),
            params: vec![format!(
                "{}@depth{}@{}",
                SETTINGS.binance.currency_pair, SETTINGS.binance.depth, SETTINGS.binance.latency
            )],
            id,
        };

        if let Err(err) = write
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
            .await
        {
            eprintln!("Could not subscribe to Binance: {}", err);
        }
    }

    async fn unsubscribe_from_partial_book_depth(write: &mut WriteSink, id: usize) {
        let request = Request {
            method: String::from("UNSUBSCRIBE"),
            params: vec![format!(
                "{}@depth{}@{}",
                SETTINGS.binance.currency_pair, SETTINGS.binance.depth, SETTINGS.binance.latency
            )],
            id,
        };

        if let Err(err) = write
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
            .await
        {
            eprintln!("Could not unsubscribe from Binance: {}", err);
        }
    }
}

#[derive(Serialize)]
struct Request {
    method: String,
    params: Vec<String>,
    id: usize,
}

#[derive(Deserialize)]
struct Response {
    result: Option<String>,
    id: usize,
}

#[allow(dead_code, non_snake_case)]
#[derive(Deserialize)]
struct PartialBookDepth {
    lastUpdateId: u128,
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}
