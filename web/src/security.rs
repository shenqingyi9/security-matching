use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use futures::StreamExt;
use gloo_net::eventsource::futures::EventSource;
use gloo_utils::format::JsValueSerdeExt;
use plotters::prelude::{Circle, GREEN, IntoFont};
use plotters::prelude::{ChartBuilder, IntoDrawingArea, SVGBackend, RED};
use plotters::series::LineSeries;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use yew::{html, AttrValue, Component, Context, Html, Properties};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Dir {
    Buy,
    Sell,
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub api: AttrValue,
}

pub enum Msg {
    Init((Security, EventSource)),
    Trade((Option<Dir>, Decimal, i64)),
    Order((Dir, Decimal, i64)),
}

#[derive(Deserialize, Default)]
pub struct Picture {
    bids: BTreeMap<Decimal, i64>,
    offers: BTreeMap<Decimal, i64>,
    price_call: Option<f64>,
}

#[derive(Default)]
pub struct Security {
    pic: Picture,
    recs: Vec<AttrValue>,
    es: Option<EventSource>,
}

impl Component for Security {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let api = ctx.props().api.clone();
        ctx.link().send_future(async move {
            let mut es = EventSource::new(&api).unwrap();
            Msg::Init({
                let mut pic = Picture::default();
                let mut init = es.subscribe("message").unwrap();
                if let Some(Ok((_, event))) = init.next().await {
                    pic = serde_json::from_str(&event.data().into_serde::<String>().unwrap()).unwrap();
                }
                (
                    Self {
                        pic,
                        recs: Vec::new(),
                        es: None,
                    },
                    es,
                )
            })
        });

        Self::default()
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Init((security, mut es)) => {
                ctx.link()
                    .send_stream(es.subscribe("trade").unwrap().map(Result::ok).filter_map(|event| async {
                        event.map(|(_, event)| Msg::Trade(serde_json::from_str(&event.data().into_serde::<String>().unwrap()).unwrap()))
                    }));
                ctx.link()
                    .send_stream(es.subscribe("order").unwrap().map(Result::ok).filter_map(|event| async {
                        event.map(|(_, event)| Msg::Order(serde_json::from_str(&event.data().into_serde::<String>().unwrap()).unwrap()))
                    }));
                *self = security;
                self.es = Some(es);
            }
            Msg::Trade((dir, price, vol)) => {
                
                if let Some(_) = dir {
                    let mut bid = self.pic.bids.last_entry().unwrap();
                    *bid.get_mut() -= vol;
                    if *bid.get() == 0 {
                        bid.remove();
                    }
                    let mut offer = self.pic.offers.first_entry().unwrap();
                    *offer.get_mut() -= vol;
                    if *offer.get() == 0 {
                        offer.remove();
                    }
                } else {
                    self.pic.price_call = price.to_f64();
                    {
                        let mut vol = vol;
                        while let Some(mut bid) = self.pic.bids.last_entry() {
                            let quantity = *bid.get();
                            if quantity > vol {
                                *bid.get_mut() -= vol;
                                break;
                            } else {
                                bid.remove();
                                if quantity == vol {
                                    break;
                                } else {
                                    vol -= quantity;
                                }
                            }
                        }
                    }
                    {
                        let mut vol = vol;
                        while let Some(mut offer) = self.pic.offers.first_entry() {
                            let quantity = *offer.get();
                            if quantity > vol {
                                *offer.get_mut() -= vol;
                                break;
                            } else {
                                offer.remove();
                                if quantity == vol {
                                    break;
                                } else {
                                    vol -= quantity;
                                }
                            }
                        }
                    }
                }
                self.recs
                    .push(serde_json::to_string(&("Trade", dir, price, vol)).unwrap().into());
            }
            Msg::Order((dir, price, count)) => {
                match match dir {
                    Dir::Buy => self.pic.bids.entry(price),
                    Dir::Sell => self.pic.offers.entry(price),
                } {
                    Entry::Vacant(orders) => {
                        if count > 0 {
                            orders.insert(count);
                        }
                    }
                    Entry::Occupied(mut order) => {
                        *order.get_mut() += count;
                        if count < 0 {
                            self.recs.push(serde_json::to_string(&("Cancel", dir, price, count)).unwrap().into());
                            if *order.get() == 0 {
                                order.remove();
                            }
                        }
                    }
                }
            }
        }
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let min = match (
            self.pic.bids.first_key_value(),
            self.pic.offers.first_key_value(),
        ) {
            (Some((&mb, _)), Some((&mo, _))) => Some(std::cmp::min(mb, mo)),
            (Some((&mb, _)), _) => Some(mb),
            (_, Some((&mo, _))) => Some(mo),
            _ => None,
        };
        let max = match (
            self.pic.bids.last_key_value(),
            self.pic.offers.last_key_value(),
        ) {
            (Some((&mb, _)), Some((&mo, _))) => Some(std::cmp::max(mb, mo)),
            (Some((&mb, _)), _) => Some(mb),
            (_, Some((&mo, _))) => Some(mo),
            _ => None,
        };
        let mut svg = String::new();
        if let (Some(min), Some(max)) = (min, max) {
            let mut mb = 0;
            let mut mo = 0;
            let mut bids = Vec::new();
            let mut offers = Vec::new();
            for (&k, &v) in self.pic.bids.iter().rev() {
                mb += v;
                bids.push((k.to_f64().unwrap(), mb));
            }
            for (&k, &v) in &self.pic.offers {
                mo += v;
                offers.push((k.to_f64().unwrap(), mo));
            }

            let drawing_area = SVGBackend::with_string(&mut svg, (640, 480)).into_drawing_area();
            let mut chart = ChartBuilder::on(&drawing_area)
                .caption(self.pic.price_call.map(|p| p.to_string()).unwrap_or_default(), ("sans-serif", 50).into_font())
                .margin(5)
                .x_label_area_size(60)
                .y_label_area_size(30)
                .build_cartesian_2d(
                    min.to_f64().unwrap() - 0.02..max.to_f64().unwrap() + 0.02,
                    0i64..std::cmp::max(mb, mo) + 100,
                )
                .unwrap();
            chart.configure_mesh().draw().unwrap();
            chart.draw_series(bids.iter().map(|&c| Circle::new(c, 3, &GREEN))).unwrap();
            chart.draw_series(LineSeries::new(bids, &GREEN)).unwrap();
            chart.draw_series(offers.iter().map(|&c| Circle::new(c, 3, &RED))).unwrap();
            chart.draw_series(LineSeries::new(offers, &RED)).unwrap();
        }
        let svg = Html::from_html_unchecked(svg.into());
        html! {
            <div>
                <p>{svg}</p>

                <p><ol>
                    {for self.pic.bids.iter().map(|(price, vol)| format!("Bid: {}, {}\n", price, vol))}
                </ol></p>

                <p><ol>
                    {for self.pic.offers.iter().map(|(price, vol)| format!("Offer: {}, {}\n", price, vol))}
                </ol></p>
                
                <p><ul>
                    {for self.recs.iter()}
                </ul></p>
            </div>
        }
    }
}
