use crate::deal::Deal;
use crate::msg::{MsgBody, MsgBox};
use crate::period::Period;
use crate::security::Security;
use chrono::Utc;
use dashmap::mapref::one::RefMut;
use dashmap::DashMap;
use entity::sea_orm_active_enums::Dir;
use entity::{order, rec, req};
use futures::{Stream, StreamExt};
use implicit_clone::sync::IString;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, ModelTrait, Select, TransactionTrait,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub engine: Arc<DashMap<Arc<str>, Arc<Security>>>,
    pub period: Arc<RwLock<Period>>,
    pub msg_box: Arc<MsgBox>,
}

async fn update_order(conn: &impl ConnectionTrait, model: order::Model) {
    match model.quantity {
        0 => {
            model.delete(conn).await.unwrap();
        }
        _ => {
            let mut model = model.into_active_model();
            model.reset(order::Column::Quantity);
            model.update(conn).await.unwrap();
        }
    }
}

pub async fn trade(conn: &impl ConnectionTrait, code: String, deal: Deal) -> rec::Model {
    let buyer_id = req::Entity::find_by_id(deal.value.seq_bid)
        .one(conn)
        .await
        .unwrap()
        .unwrap()
        .id;
    let seller_id = req::Entity::find_by_id(deal.value.seq_offer)
        .one(conn)
        .await
        .unwrap()
        .unwrap()
        .id;
    let mut bid = order::Entity::find_by_id(deal.value.seq_bid)
        .one(conn)
        .await
        .unwrap()
        .unwrap();
    let mut offer = order::Entity::find_by_id(deal.value.seq_offer)
        .one(conn)
        .await
        .unwrap()
        .unwrap();
    bid.quantity -= deal.value.quantity;
    update_order(conn, bid).await;
    offer.quantity -= deal.value.quantity;
    update_order(conn, offer).await;
    rec::ActiveModel {
        code: ActiveValue::Set(code),
        buyer_id: ActiveValue::Set(buyer_id),
        seller_id: ActiveValue::Set(seller_id),
        price: ActiveValue::Set(deal.price),
        quantity: ActiveValue::Set(deal.value.quantity),
        ..Default::default()
    }
    .insert(conn)
    .await
    .unwrap()
}

pub fn msg_deal(rec: &rec::Model, deal: Deal) -> [MsgBody; 2] {
    [
        MsgBody {
            name: IString::Static("trade"),
            data: Arc::new(order::Model {
                seq: deal.value.seq_bid,
                code: rec.code.clone(),
                dir: Dir::Buy,
                price: rec.price,
                quantity: rec.quantity,
            }),
            happened_at: Utc::now().fixed_offset(),
        },
        MsgBody {
            name: IString::Static("trade"),
            data: Arc::new(order::Model {
                seq: deal.value.seq_offer,
                code: rec.code.clone(),
                dir: Dir::Sell,
                price: rec.price,
                quantity: rec.quantity,
            }),
            happened_at: Utc::now().fixed_offset(),
        },
    ]
}

impl AppState {
    pub async fn restore(db: DatabaseConnection) -> Self {
        let state = Self {
            db,
            engine: Default::default(),
            period: Arc::new(RwLock::new(Period::Suspense)),
            msg_box: Default::default(),
        };
        let mut orders = order::Entity::find().stream(&state.db).await.unwrap();
        while let Some(Ok(order)) = orders.next().await {
            let state = state.clone();
            state
                .entry_or_default(Arc::from(order.code.clone()))
                .push(&order)
                .await;
        }
        let mut msgs = entity::msg::Entity::find().stream(&state.db).await.unwrap();
        while let Some(Ok(msg)) = msgs.next().await {
            let entity::msg::Model {
                id,
                event_type,
                data,
                happened_at,
                ..
            } = msg;
            let name = IString::from(event_type);
            let data = Arc::new(data);
            state.msg_box.unsent.entry(id).or_default().push(MsgBody {
                name,
                data,
                happened_at,
            })
        }
        state.clone()
    }

    pub async fn send(&self, id: i64, event: MsgBody) {
        if let Some(MsgBody {
            name,
            data,
            happened_at,
        }) = self.msg_box.send(id, event)
        {
            entity::msg::ActiveModel {
                id: ActiveValue::Set(id),
                event_type: ActiveValue::Set(name.to_string()),
                data: ActiveValue::Set(serde_json::json!(data)),
                happened_at: ActiveValue::Set(happened_at),
                ..Default::default()
            }
            .insert(&self.db)
            .await
            .unwrap();
        }
    }

    pub fn entry_or_default(&self, code: Arc<str>) -> RefMut<'_, Arc<str>, Arc<Security>> {
        let state = self.clone();
        self.engine
            .entry(code.clone())
            .or_insert(Arc::new(Security::new(self.period.clone(), move |deal| {
                let code = code.to_string();
                let state = state.clone();
                async move {
                    let txn = state.db.begin().await.unwrap();
                    let rec = trade(&txn, code, deal).await;
                    txn.commit().await.unwrap();
                    let [msg_buyer, msg_seller] = msg_deal(&rec, deal);
                    state.send(rec.buyer_id, msg_buyer).await;
                    state.send(rec.seller_id, msg_seller).await;
                }
            })))
    }

    pub async fn stream_query<E: EntityTrait>(
        &self,
        query: Select<E>,
    ) -> impl Stream<Item = Result<E::Model, DbErr>> {
        let db = self.db.clone();
        async_stream::stream! {
            for await model in query.stream(&db).await.unwrap() {
                yield model;
            }
        }
    }
}
