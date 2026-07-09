//! Communication domain events (hand-authored, user-owned) — the public inbound-routing surface.
//!
//! The channel gateway's whole point is to turn an inbound message into a domain event that CRM (capture a
//! lead), support (open/append an issue), etc. subscribe to **over the event bus — not a driven call**.
//! `MessageReceived` is that event; `MessageDelivered`/`MessageFailed` report the fate of an outbound send.
//! A consuming service supplies the sink (bus, outbox, …).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An inbound message was recorded on a thread — the signal CRM/support route on. Carries everything a
/// consumer needs to act (who it's from, what it concerns, the body) so it never re-queries this module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageReceived {
    pub message_id: Uuid,
    pub thread_id: Uuid,
    pub company_id: Uuid,
    pub channel: String,
    pub party_id: Option<Uuid>,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub address_from: Option<String>,
    pub body: String,
}

/// The communication domain-event union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum CommunicationEvent {
    MessageReceived(MessageReceived),
    MessageDelivered { message_id: Uuid, external_id: String },
    MessageFailed { message_id: Uuid, reason: String },
}

/// Sink the write path publishes to. A consuming service supplies its own (bus, outbox, …).
pub trait CommunicationEventSink: Send + Sync {
    fn publish(&self, event: &CommunicationEvent);
}

/// A no-op/logging sink for tests and single-process composition.
#[derive(Debug, Default, Clone)]
pub struct LoggingSink;

impl CommunicationEventSink for LoggingSink {
    fn publish(&self, event: &CommunicationEvent) {
        tracing::info!(?event, "communication event");
    }
}
