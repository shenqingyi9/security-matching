use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::future::Future;
use std::ops::DerefMut;
use std::sync::Arc;

use tokio::sync::{broadcast, oneshot, watch, RwLock};
use tokio::task;
use tokio::time::{self, Duration};

use entity::order;
use entity::sea_orm_active_enums::Dir;

use crate::book::{Book, Picture};
use crate::deal::{Deal, DealCall, DealValue};
use crate::period::Period;

#[derive(Clone)]
pub struct Security {
    book: Arc<RwLock<Book>>,
    que: Arc<RwLock<VecDeque<Arc<order::Model>>>>,
    watcher: watch::Sender<bool>,
    pub bc_deal: broadcast::Sender<(Option<Dir>, Decimal, i64)>,
    pub bc_order: broadcast::Sender<(Dir, Decimal, i64)>,
}

impl Security {
    pub async fn push(&self, order: &order::Model) {
        self.book.write().await.insert(order);
    }

    pub fn new<FutT: Send + Future<Output = ()>>(
        period: Arc<RwLock<Period>>,
        mut trade: impl FnMut(Deal) -> FutT + Send + 'static,
    ) -> Self {
        let (tx, mut rx) = watch::channel(true);
        let deal_maker = Self {
            book: Default::default(),
            que: Default::default(),
            watcher: tx,
            bc_deal: broadcast::Sender::new(1024),
            bc_order: broadcast::Sender::new(1024),
        };
        task::spawn({
            let deal_maker = deal_maker.clone();
            async move {
                while !*rx.wait_for(|is_empty| !is_empty).await.unwrap() {
                    while let Some(order) = {
                        let mut que = deal_maker.que.write().await;
                        let pop = que.pop_front();
                        if pop.is_none() {
                            deal_maker.watcher.send(true).unwrap();
                        }
                        pop
                    } {
                        let dir = order.dir;
                        deal_maker.book.write().await.insert(&order);
                        deal_maker
                            .bc_order
                            .send((order.dir, order.price, order.quantity))
                            .unwrap_or_default();
                        while let Some(deal) = match *period.read().await {
                            Period::Continuous => {
                                deal_maker.book.write().await.matches(match dir {
                                    Dir::Buy => |_, price| price,
                                    Dir::Sell => |price, _| price,
                                })
                            }
                            _ => None,
                        } {
                            trade(deal).await;
                            deal_maker
                                .bc_deal
                                .send((Some(dir), deal.price, deal.value.quantity))
                                .unwrap_or_default();
                        }
                    }
                }
            }
        });
        deal_maker
    }

    pub async fn place<FutI: Future<Output = ()>>(
        &self,
        order: Arc<order::Model>,
        store: impl FnOnce(Arc<order::Model>) -> FutI,
    ) {
        let mut que = self.que.write().await;
        store(order.clone()).await;
        que.push_back(order);
        self.watcher.send(false).unwrap();
    }

    pub async fn cancel<FutC: Future<Output = ()>>(
        &self,
        order: &order::Model,
        cancel: impl FnOnce(Option<i64>) -> FutC,
    ) {
        let mut que = self.que.write().await;
        cancel(
            if let Ok(i) = que.binary_search_by_key(&order.seq, |order| order.seq) {
                que.remove(i).map(|order| order.quantity)
            } else {
                let quantity = self.book.write().await.remove(order);
                if let Some(quantity) = quantity {
                    self.bc_order
                    .send((order.dir, order.price, -quantity))
                    .unwrap_or_default();
                }
                quantity
            },
        )
        .await;
    }

    pub async fn calc(&self) -> Option<DealCall> {
        time::sleep(Duration::from_secs(3)).await;
        self.watcher
            .subscribe()
            .wait_for(|&is_empty| is_empty)
            .await
            .unwrap();
        let mut book = self.book.write().await;
        let (tx, rx) = oneshot::channel();
        let deal;
        *book = {
            let mut book = std::mem::take(book.deref_mut());
            rayon::spawn({
                || {
                    let deal = book.calc();
                    tx.send((book, deal)).unwrap()
                }
            });
            (book, deal) = rx.await.unwrap();
            book
        };
        if let Some(DealCall { price, values }) = &deal {
            self.bc_deal.send((None, *price, values.iter().map(|DealValue {quantity, ..}| *quantity).sum())).unwrap_or_default();
        }
        deal
    }

    pub async fn view(&self) -> Picture {
        self.book.read().await.view()
    }
}
