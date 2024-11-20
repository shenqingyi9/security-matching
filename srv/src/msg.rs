use chrono::{DateTime, FixedOffset};
use dashmap::DashMap;
use implicit_clone::sync::IString;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Serialize, Clone)]
pub struct MsgBody {
    pub name: IString,
    pub data: Arc<dyn erased_serde::Serialize + Send + Sync>,
    pub happened_at: DateTime<FixedOffset>,
}

#[derive(Default)]
pub struct MsgBox {
    addrs: DashMap<i64, UnboundedSender<MsgBody>>,
    pub unsent: DashMap<i64, Vec<MsgBody>>,
}

impl MsgBox {
    pub fn send(&self, id: i64, body: MsgBody) -> Option<MsgBody> {
        if let Some(tx) = self.addrs.get_mut(&id) {
            tx.send(body).err().map(|SendError(msg)| msg)
        } else {
            Some(body)
        }
        .map(|body| {
            self.unsent.entry(id).or_default().push(body.clone());
            body
        })
    }

    pub fn online(&self, id: i64, tx: UnboundedSender<MsgBody>) -> Option<(i64, Vec<MsgBody>)> {
        self.addrs.insert(id, tx);
        self.unsent.remove(&id)
    }
}
