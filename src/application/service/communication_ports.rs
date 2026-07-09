//! Outbound channel port (hand-authored, user-owned) — the seam to the outside world.
//!
//! Sending a message actually leaves the system through a `ChannelPort`: a WhatsApp Business API adapter
//! (the Indonesia edge), an email gateway, an SMS provider. The module never imports a provider SDK — a
//! composing service wires the real adapter behind this trait; tests supply a fake. Zero normal Cargo edge
//! to any transport.

use serde::{Deserialize, Serialize};

/// A request to hand one message to a channel provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutboundSend {
    pub channel: String, // whatsapp | email | sms
    pub to: String,
    pub body: String,
}

/// The provider accepted the message and assigned it an id (the delivery-receipt correlation key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelAck {
    pub external_id: String,
}

/// The provider rejected the send (bad number, template not approved, rate limit …). `code` is stable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelRejected {
    pub code: String,
    pub message: String,
}

/// The outbound seam. A composing service implements it over a real WhatsApp/email/SMS provider.
#[async_trait::async_trait]
pub trait ChannelPort: Send + Sync {
    async fn send(&self, send: &OutboundSend) -> Result<ChannelAck, ChannelRejected>;
}
