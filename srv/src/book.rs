use crate::deal::{Deal, DealCall, DealValue};
use entity::{order, sea_orm_active_enums::Dir};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::btree_map::{BTreeMap, Entry};

#[derive(Default, Clone, Debug)]
pub struct Vol {
    pub prices: BTreeMap<i64, i64>,
    pub sum: i64,
}

#[derive(Default, Clone, Debug)]
pub struct Book {
    pub bids: BTreeMap<Decimal, Vol>,
    pub offers: BTreeMap<Decimal, Vol>,
    pub price_call: Option<Decimal>,
}

impl Book {
    pub fn insert(&mut self, order: &order::Model) {
        let vol = match order.dir {
            Dir::Buy => self.bids.entry(order.price).or_default(),
            Dir::Sell => self.offers.entry(order.price).or_default(),
        };
        vol.sum += order.quantity;
        vol.prices.insert(order.seq, order.quantity);
    }

    pub fn matches(&mut self, get_price: impl Fn(Decimal, Decimal) -> Decimal) -> Option<Deal> {
        Option::zip(self.bids.last_entry(), self.offers.first_entry())
            .and_then(|(mut bid_vol, mut offer_vol)| {
                (bid_vol.key() >= offer_vol.key()).then(|| {
                    let price = get_price(*bid_vol.key(), *offer_vol.key());
                    Option::zip(
                        bid_vol.get_mut().prices.first_entry(),
                        offer_vol.get_mut().prices.first_entry(),
                    )
                    .map(|(mut bid, mut offer)| {
                        let quantity = *std::cmp::min(bid.get(), offer.get());
                        *bid.get_mut() -= quantity;
                        *offer.get_mut() -= quantity;
                        let seq_bid = *bid.key();
                        let seq_offer = *offer.key();
                        if *bid.get_mut() == 0 {
                            bid.remove();
                        }
                        if *offer.get_mut() == 0 {
                            offer.remove();
                        }
                        DealValue {
                            seq_bid,
                            seq_offer,
                            quantity,
                        }
                    })
                    .map(|value| {
                        bid_vol.get_mut().sum -= value.quantity;
                        offer_vol.get_mut().sum -= value.quantity;
                        let remain_bid = bid_vol.get().sum;
                        if remain_bid == 0 {
                            bid_vol.remove();
                        }
                        let remain_offer = offer_vol.get().sum;
                        if remain_offer == 0 {
                            offer_vol.remove();
                        }
                        Deal {
                            price,
                            value,
                        }
                    })
                })
            })
            .flatten()
    }

    pub fn calc(&mut self) -> Option<DealCall> {
        let values = std::iter::from_fn(|| {
            self.matches(|price_bid, price_offer| (price_bid + price_offer) / Decimal::TWO)
                .map(|deal| {
                    self.price_call = Some(deal.price);
                    deal.value
                })
        })
        .collect();
        self.price_call.map(|price| DealCall { price, values })
    }

    pub fn remove(&mut self, order: &order::Model) -> Option<i64> {
        match match order.dir {
            Dir::Buy => self.bids.entry(order.price),
            Dir::Sell => self.offers.entry(order.price),
        } {
            Entry::Vacant(_) => None,
            Entry::Occupied(mut vol) => {
                vol.get_mut().sum -= order.quantity;
                let quantity = vol.get_mut().prices.remove(&order.seq);
                if vol.get_mut().sum == 0 {
                    vol.remove();
                }
                quantity
            }
        }
    }
}

#[derive(Serialize, Default, Clone)]
pub struct Picture {
    bids: BTreeMap<Decimal, i64>,
    offers: BTreeMap<Decimal, i64>,
    price_call: Option<Decimal>,
}

impl Book {
    pub fn view(&self) -> Picture {
        Picture {
            bids: self.bids.iter().map(|(k, q)| (*k, q.sum)).collect(),
            offers: self.offers.iter().map(|(k, q)| (*k, q.sum)).collect(),
            price_call: self.price_call,
        }
    }
}
