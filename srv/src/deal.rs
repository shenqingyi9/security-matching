use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Serialize, Copy, Clone, Debug)]
pub struct DealValue {
    pub seq_bid: i64,
    pub seq_offer: i64,
    pub quantity: i64,
}

#[derive(Clone, Default, Debug)]
pub struct DealCall {
    pub price: Decimal,
    pub values: Vec<DealValue>,
}

#[derive(Serialize, Copy, Clone, Debug)]
pub struct Deal {
    pub price: Decimal,
    pub value: DealValue,
}
