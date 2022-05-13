use futures_util::{stream::SplitStream, StreamExt};
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

type ReadStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

#[derive(Debug)]
pub enum LoopState {
    Continue,
    Break,
}

pub async fn read_from_stream<T>(read: &mut ReadStream) -> Result<T, LoopState>
where
    T: DeserializeOwned,
{
    if let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            if let Message::Text(text) = msg {
                match serde_json::from_str::<T>(text.as_str()) {
                    Ok(response) => Ok(response),
                    Err(err) => {
                        eprintln!("Error parsing message: {}\n{}", err, text);
                        Err(LoopState::Continue)
                    }
                }
            } else {
                Err(LoopState::Continue)
            }
        } else {
            eprintln!("server went away");
            Err(LoopState::Break)
        }
    } else {
        eprintln!("no message");
        Err(LoopState::Break)
    }
}
