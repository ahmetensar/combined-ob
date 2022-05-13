use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    exchange::util::{read_from_stream, LoopState},
    msg::{Level, Levels},
    shutdown, SETTINGS,
};

type WriteSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

pub struct Bitstamp {
    pub levels_tx: mpsc::Sender<Levels>,
    pub shutdown_rx: shutdown::Receiver,
}

impl Bitstamp {
    const EXCHANGE: &'static str = "Bitstamp";
    const BITSTAMP_WSS: &'static str = "wss://ws.bitstamp.net";

    pub async fn connect(&mut self) {
        let url = url::Url::parse(Self::BITSTAMP_WSS).unwrap();
        let (ws_stream, _) = connect_async(url)
            .await
            .expect("Failed to connect to Bitstamp");

        let (mut write, mut read) = ws_stream.split();

        Self::subscribe_to_orderbook(&mut write).await;

        let res = read_from_stream::<Response>(&mut read).await.unwrap();
        assert_eq!("bts:subscription_succeeded", res.event);

        loop {
            tokio::select! {
                res = read_from_stream::<Data<Orderbook>>(&mut read) => {
                    match res {
                        Ok(data) => {
                            self.process_orderbook(data.data).await;
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

        Self::unsubscribe_from_orderbook(&mut write).await;

        let _ = write.close();

        println!("Exiting bitstamp...");
    }

    async fn process_orderbook(&self, orderbook: Orderbook) {
        let bids: Vec<_> = orderbook
            .bids
            .into_iter()
            .map(|b| Level {
                exchange: Self::EXCHANGE,
                price: b[0].parse().unwrap(),
                amount: b[1].parse().unwrap(),
            })
            .collect();

        let asks: Vec<_> = orderbook
            .asks
            .into_iter()
            .map(|b| Level {
                exchange: Self::EXCHANGE,
                price: b[0].parse().unwrap(),
                amount: b[1].parse().unwrap(),
            })
            .collect();

        let levels = Levels {
            exchange: Self::EXCHANGE,
            bids,
            asks,
        };

        if let Err(err) = self.levels_tx.send(levels).await {
            eprintln!("Error sending message: {}", err);
        }
    }

    async fn subscribe_to_orderbook(write: &mut WriteSink) {
        let channel = format!("order_book_{}", SETTINGS.bitstamp.currency_pair);
        let request = Request::<PublicChannel> {
            event: String::from("bts:subscribe"),
            data: PublicChannel { channel },
        };

        if let Err(err) = write
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
            .await
        {
            eprintln!("Could not subscribe to Bitstamp: {}", err);
        }
    }

    async fn unsubscribe_from_orderbook(write: &mut WriteSink) {
        let channel = format!("order_book_{}", SETTINGS.bitstamp.currency_pair);
        let request = Request::<PublicChannel> {
            event: String::from("bts:unsubscribe"),
            data: PublicChannel { channel },
        };

        if let Err(err) = write
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
            .await
        {
            eprintln!("Could not unsubscribe from Bitstamp: {}", err);
        }
    }
}

#[derive(Serialize)]
struct Request<T> {
    event: String,
    data: T,
}

#[derive(Serialize)]
struct PublicChannel {
    channel: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Response {
    event: String,
    channel: String,
    data: Value,
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Orderbook {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
    timestamp: String,
    microtimestamp: String,
}
