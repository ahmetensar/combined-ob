use super::Level;

pub struct Levels {
    pub exchange: &'static str,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}
