//! Shared test helpers: a live pool, a fake channel provider (records sends, assigns ids or rejects),
//! and a capturing event sink so tests can assert exactly which domain events were published.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use backbone_communication::application::service::communication_events::{
    CommunicationEvent, CommunicationEventSink,
};
use backbone_communication::application::service::communication_ports::{
    ChannelAck, ChannelPort, ChannelRejected, OutboundSend,
};
use sqlx::PgPool;

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_communication".into())
}
pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}

/// A fake channel provider. Records every send; assigns a deterministic external id, unless armed to
/// reject (to exercise the failed-send path).
#[derive(Clone, Default)]
pub struct FakeChannel {
    pub sends: Arc<Mutex<Vec<OutboundSend>>>,
    pub reject: Arc<Mutex<Option<(String, String)>>>, // (code, message)
}
impl FakeChannel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn rejecting(code: &str, message: &str) -> Self {
        let f = Self::default();
        *f.reject.lock().unwrap() = Some((code.into(), message.into()));
        f
    }
    pub fn count(&self) -> usize {
        self.sends.lock().unwrap().len()
    }
}
#[async_trait::async_trait]
impl ChannelPort for FakeChannel {
    async fn send(&self, send: &OutboundSend) -> Result<ChannelAck, ChannelRejected> {
        self.sends.lock().unwrap().push(send.clone());
        if let Some((code, message)) = self.reject.lock().unwrap().clone() {
            return Err(ChannelRejected { code, message });
        }
        // Globally unique so parallel tests never collide on the (channel, external_id) unique index.
        Ok(ChannelAck { external_id: format!("prov-{}", uuid::Uuid::new_v4()) })
    }
}

/// Captures published events so tests can count and inspect them.
#[derive(Clone, Default)]
pub struct CapturingSink {
    pub events: Arc<Mutex<Vec<CommunicationEvent>>>,
}
impl CapturingSink {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn received(&self) -> usize {
        self.events.lock().unwrap().iter()
            .filter(|e| matches!(e, CommunicationEvent::MessageReceived(_)))
            .count()
    }
    pub fn delivered(&self) -> usize {
        self.events.lock().unwrap().iter()
            .filter(|e| matches!(e, CommunicationEvent::MessageDelivered { .. }))
            .count()
    }
    pub fn last_received(&self) -> backbone_communication::application::service::communication_events::MessageReceived {
        self.events.lock().unwrap().iter().rev()
            .find_map(|e| match e { CommunicationEvent::MessageReceived(m) => Some(m.clone()), _ => None })
            .expect("a MessageReceived")
    }
}
impl CommunicationEventSink for CapturingSink {
    fn publish(&self, event: &CommunicationEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}
