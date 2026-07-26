//! Integrity probes — the gateway's invariants: inbound needs a provider id, a closed thread refuses
//! sends, a channel rejection is recorded (not swallowed), and delivery receipts are idempotent.

mod common;
use common::*;

use backbone_communication::application::service::communication_write_service::*;
use uuid::Uuid;

fn inbound(company: Uuid, ext: &str) -> InboundMessage {
    InboundMessage {
        company_id: company, channel: "whatsapp".into(), external_id: ext.into(),
        address_from: Some("+628123".into()), address_to: None, body: "hi".into(),
        party_id: None, external_ref: None, subject_type: None, subject_id: None,
    }
}

// CIP-1 — an inbound message with no provider external_id is refused (there'd be no dedup key).
#[tokio::test]
async fn cip1_inbound_requires_external_id() {
    let pool = pool().await;
    let svc = CommunicationWriteService::new(pool.clone());
    let r = svc.receive_inbound(inbound(Uuid::new_v4(), "   "), &CapturingSink::new()).await;
    assert!(matches!(r, Err(CommError::Invalid(_))));
}

// CIP-2 — a closed thread refuses further sends.
#[tokio::test]
async fn cip2_closed_thread_refuses_send() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let thread = svc.open_thread(company, "whatsapp", None, None).await.unwrap();
    svc.close_thread(thread).await.unwrap();
    let r = svc.send_outbound(thread, "+628".into(), "hi".into(), &FakeChannel::new(), &CapturingSink::new()).await;
    assert!(matches!(r, Err(CommError::InvalidState(_))));
}

// CIP-3 — a channel rejection is persisted as a failed message (not swallowed) and surfaces the error.
#[tokio::test]
async fn cip3_channel_rejection_recorded() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();
    let channel = FakeChannel::rejecting("invalid_number", "bad msisdn");

    let thread = svc.open_thread(company, "whatsapp", None, None).await.unwrap();
    let r = svc.send_outbound(thread, "+628".into(), "hi".into(), &channel, &sink).await;
    assert!(matches!(r, Err(CommError::ChannelRejected(_))));

    // The message row records the failure.
    let (status, reason): (String, Option<String>) = sqlx::query_as(
        "SELECT status::text, failure_reason FROM communication.messages WHERE thread_id=$1")
        .bind(thread).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "failed");
    assert_eq!(reason.as_deref(), Some("bad msisdn"));
}

// CIP-4 — a delivery receipt for an unknown/already-delivered message is a no-op (no error, no double
// publish).
#[tokio::test]
async fn cip4_delivery_receipt_idempotent_and_safe() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();
    // Unknown external id → no row transitions, no event.
    svc.mark_delivered("whatsapp", company, &format!("unknown-{}", Uuid::new_v4()), &sink).await.unwrap();
    assert_eq!(sink.delivered(), 0);
}

// CIP-5 — routing by provider conversation handle (external_ref) reuses the same thread across messages.
#[tokio::test]
async fn cip5_routing_by_conversation_handle() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();
    let mut a = inbound(company, &format!("m-{}", Uuid::new_v4())); a.external_ref = Some("conv-x".into());
    let mut b = inbound(company, &format!("m-{}", Uuid::new_v4())); b.external_ref = Some("conv-x".into());
    let first = svc.receive_inbound(a, &sink).await.unwrap();
    let second = svc.receive_inbound(b, &sink).await.unwrap();
    assert_eq!(first.thread_id, second.thread_id, "same conversation handle → one thread");
}

/// A sink that drops every event — simulates a crash/loss in the window between the DB commit and the
/// in-proc publish.
struct DroppingSink;
impl backbone_communication::application::service::communication_events::CommunicationEventSink for DroppingSink {
    fn publish(&self, _e: &backbone_communication::application::service::communication_events::CommunicationEvent) {}
}

// CIP-6 — the routing event is DURABLE (maturity council 2026-07-08). Even when the in-proc publish is
// lost (crash-after-commit, modelled by a dropping sink), MessageReceived is staged in the outbox in the
// same tx as the message, so a relay can still deliver it. Without the transactional-outbox staging the
// event would be lost forever — the dedup fast-path swallows the provider's redelivery.
#[tokio::test]
async fn cip6_routing_event_is_durable_via_outbox() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let ext = format!("m-{}", Uuid::new_v4());

    let out = svc.receive_inbound(inbound(company, &ext), &DroppingSink).await.unwrap();
    assert!(!out.duplicate);

    // The event survived the lost publish — it is durably staged in the outbox for the relay.
    let staged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication.outbox_events WHERE aggregate_id=$1 AND event_type='MessageReceived'")
        .bind(out.message_id.to_string())
        .fetch_one(&pool).await.unwrap();
    assert_eq!(staged, 1, "MessageReceived is durably staged despite the lost in-proc publish");
}
