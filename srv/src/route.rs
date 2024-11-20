use crate::deal::{Deal, DealCall};
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Sse};
use axum::{routing, Router};
use axum_streams::StreamBodyAs;
use chrono::Utc;
use entity::{order, req};
use futures::{stream, StreamExt};
use implicit_clone::sync::IString;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use std::sync::Arc;

use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::wrappers::{BroadcastStream, UnboundedReceiverStream};

use crate::msg::MsgBody;
use crate::period::Period;
use crate::state;
use crate::state::AppState;

async fn msg(State(state): State<AppState>, Query(id): Query<i64>) -> impl IntoResponse {
    let (tx, rx) = unbounded_channel::<MsgBody>();
    let unsent = state.msg_box.online(id, tx);
    Sse::new(
        stream::once(async { Event::default().json_data(unsent) }).chain(
            UnboundedReceiverStream::new(rx)
                .map(|event: MsgBody| Event::default().event(event.name).json_data(event.data)),
        ),
    )
    .keep_alive(KeepAlive::default())
}

async fn place(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(mut order): Json<order::Model>,
) -> impl IntoResponse {
    match *state.period.read().await {
        Period::Suspense => (StatusCode::FORBIDDEN, Json(None)),
        _ => {
            let seq = req::ActiveModel {
                id: ActiveValue::Set(id),
                body: ActiveValue::Set(serde_json::json!(&order)),
                ..Default::default()
            }
            .insert(&state.db)
            .await
            .unwrap()
            .seq;
            order.seq = seq;
            let order = Arc::new(order);
            state
                .entry_or_default(Arc::from(order.code.clone()))
                .place(order.clone(), {
                    let state = state.clone();
                    move |order| async move {
                        order
                            .as_ref()
                            .to_owned()
                            .into_active_model()
                            .insert(&state.db)
                            .await
                            .unwrap();
                    }
                })
                .await;

            (StatusCode::CREATED, Json(Some(seq)))
        }
    }
}

async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(seq): Json<i64>,
) -> impl IntoResponse {
    match *state.period.read().await {
        Period::Call | Period::Suspense => StatusCode::FORBIDDEN,
        _ => {
            if let Some(order) = order::Entity::find_by_id(seq).one(&state.db).await.unwrap() {
                let code = Arc::from(order.code.clone());
                if let Some(book) = state.engine.get(&code) {
                    let state = state.clone();
                    book.value()
                        .cancel(&order, {
                            let order = order.clone();
                            move |quantity| async move {
                                let txn = state.db.begin().await.unwrap();
                                let data = serde_json::json!({
                                "cancel": {
                                        "seq": seq,
                                        "quantity": quantity
                                    }
                                });
                                req::ActiveModel {
                                    id: ActiveValue::Set(id),
                                    body: ActiveValue::Set(data.clone()),
                                    ..Default::default()
                                }
                                .insert(&txn)
                                .await
                                .unwrap();
                                order.delete(&txn).await.unwrap();
                                txn.commit().await.unwrap();
                                state
                                    .send(
                                        id,
                                        MsgBody {
                                            name: IString::Static("Canceled"),
                                            data: Arc::from(data),
                                            happened_at: Utc::now().fixed_offset(),
                                        },
                                    )
                                    .await
                            }
                        })
                        .await
                }
            }

            StatusCode::NO_CONTENT
        }
    }
}

async fn ctrl(State(state): State<AppState>, Json(period): Json<Period>) {
    let last_period = *state.period.read().await;
    *state.period.write().await = period;
    if let (Period::Call, Period::Suspense) = (last_period, period) {
        for security in state.engine.iter() {
            let code = security.key().clone();
            let security = security.to_owned();
            let state = state.clone();
            tokio::spawn(async move {
                if let Some(DealCall { price, values }) = security.calc().await {
                    dbg!(&values);
                    let mut msg = Vec::with_capacity(values.len() * 2);
                    let txn = state.db.begin().await.unwrap();
                    for value in values {
                        let deal = Deal { price, value };
                        let rec = state::trade(&txn, code.to_string(), deal).await;
                        msg.extend(std::iter::zip(
                            [rec.buyer_id, rec.seller_id],
                            state::msg_deal(&rec, deal),
                        ));
                    }
                    txn.commit().await.unwrap();
                    for (id, body) in msg {
                        let state = state.clone();
                        tokio::spawn(async move { state.send(id, body).await });
                    }
                }
            });
        }
    }
}

async fn watch(State(state): State<AppState>, Path(code): Path<Arc<str>>) -> impl IntoResponse {
    let security = state.entry_or_default(code.clone()).value().to_owned();
    Sse::new(
        stream::once(async move {
            Event::default().json_data(state.entry_or_default(code).value().to_owned().view().await)
        })
        .chain(stream::select(
            BroadcastStream::new(security.bc_deal.subscribe())
                .map(|deal| Event::default().event("trade").json_data(deal.unwrap())),
            BroadcastStream::new(security.bc_order.subscribe())
                .map(|order| Event::default().event("order").json_data(order.unwrap())),
        )),
    )
    .keep_alive(KeepAlive::default())
}

async fn review_actions(State(state): State<AppState>, Query(id): Query<i64>) -> impl IntoResponse {
    StreamBodyAs::json_nl(
        state
            .stream_query(
                req::Entity::find()
                    .filter(req::Column::Id.eq(id))
                    .order_by_desc(req::Column::CreatedAt),
            )
            .await
            .map(|model| model.unwrap()),
    )
}

async fn view_matching(State(state): State<AppState>, Query(id): Query<i64>) -> impl IntoResponse {
    Json(
        order::Entity::find()
            .reverse_join(req::Entity)
            .filter(req::Column::Id.eq(id))
            .all(&state.db)
            .await
            .unwrap(),
    )
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/msg", routing::get(msg))
        .route("/cancel/:id", routing::delete(cancel))
        .route("/place/:id", routing::post(place))
        .route("/watch/:code", routing::get(watch))
        .route("/review_actions", routing::get(review_actions))
        .route("/view_matching", routing::get(view_matching))
        .route("/ctrl", routing::put(ctrl))
}
