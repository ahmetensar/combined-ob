use crate::server;

#[derive(Debug, Copy, Clone)]
pub struct Level {
    pub exchange: &'static str,
    pub price: f64,
    pub amount: f64,
}

impl From<&Level> for server::orderbook::Level {
    fn from(level: &Level) -> Self {
        Self {
            exchange: level.exchange.to_string(),
            price: level.price,
            amount: level.amount,
        }
    }
}
