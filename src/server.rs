pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::{shutdown, SETTINGS};

use self::orderbook::{
    orderbook_aggregator_server::{OrderbookAggregator, OrderbookAggregatorServer},
    Empty, Summary,
};

pub struct Server {
    pub shutdown_rx: shutdown::Receiver,
}

impl Server {
    pub async fn serve(&mut self, summary_tx: broadcast::Sender<Summary>) {
        let addr = SETTINGS.server.address.parse().unwrap();

        let service = OrderbookService { summary_tx };
        if let Err(err) = tonic::transport::Server::builder()
            .add_service(OrderbookAggregatorServer::new(service))
            .serve_with_shutdown(addr, self.shutdown_rx.recv())
            .await
        {
            eprintln!("grpc server failed: {}", err);
        }
        println!("Exiting server...");
    }
}

struct OrderbookService {
    summary_tx: broadcast::Sender<Summary>,
}

#[tonic::async_trait]
impl OrderbookAggregator for OrderbookService {
    type BookSummaryStream = ReceiverStream<Result<Summary, Status>>;

    async fn book_summary(
        &self,
        _: Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let (tx, rx) = mpsc::channel(SETTINGS.app.channel_capacity);

        let mut summary_rx = self.summary_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match summary_rx.recv().await {
                    Ok(summary) => {
                        if let Err(err) = tx.send(Ok(summary)).await {
                            eprintln!("book_summary rpc closed: {}", err);
                            break;
                        }
                    }
                    Err(err) => {
                        if let broadcast::error::RecvError::Lagged(_) = err {
                            eprintln!("Summary channel lagged: {}", err);
                        } else {
                            break;
                        };
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
