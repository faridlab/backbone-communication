//! Golden cases — the manufactured oracle for the channel gateway: inbound recording is exactly-once per
//! (channel, external_id), routing reuses a thread, outbound sends through the channel, and a delivery
//! receipt transitions once. Posts NO GL.

mod common;
use common::*;

use backbone_communication::application::service::communication_events::LoggingSink;
use backbone_communication::application::service::communication_write_service::*;
use uuid::Uuid;

fn inbound(company: Uuid, ext: &str, party: Option<Uuid>) -> InboundMessage {
    InboundMessage {
        company_id: company, channel: "whatsapp".into(), external_id: ext.into(),
        address_from: Some("+628123".into()), address_to: Some("+628999".into()),
        body: "Halo, mau tanya pesanan".into(), party_id: party, external_ref: Some("wa-conv-1".into()),
        subject_type: None, subject_id: None,
    }
}

// CGC-1 — an inbound message is recorded and MessageReceived is published exactly once.
#[tokio::test]
async fn cgc1_inbound_recorded_and_published() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();

    let out = svc.receive_inbound(inbound(company, &format!("m-{}", Uuid::new_v4()), None), &sink).await.unwrap();
    assert!(!out.duplicate);
    assert_eq!(sink.received(), 1);
    let ev = sink.last_received();
    assert_eq!(ev.message_id, out.message_id);
    assert_eq!(ev.thread_id, out.thread_id);
    assert_eq!(ev.body, "Halo, mau tanya pesanan");

    // A thread was opened and the message recorded inbound/received.
    let (dir, status): (String, String) = sqlx::query_as(
        "SELECT direction::text, status::text FROM communication.messages WHERE id=$1")
        .bind(out.message_id).fetch_one(&pool).await.unwrap();
    assert_eq!(dir, "inbound");
    assert_eq!(status, "received");
}

// CGC-2 — a redelivered webhook (same channel + external_id) is idempotent: no second message, no second
// publish, returns the original ids with duplicate=true.
#[tokio::test]
async fn cgc2_redelivery_is_idempotent() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();
    let ext = format!("m-{}", Uuid::new_v4());

    let first = svc.receive_inbound(inbound(company, &ext, None), &sink).await.unwrap();
    let second = svc.receive_inbound(inbound(company, &ext, None), &sink).await.unwrap();

    assert!(!first.duplicate);
    assert!(second.duplicate);
    assert_eq!(first.message_id, second.message_id, "same message");
    assert_eq!(first.thread_id, second.thread_id, "same thread — no spurious thread on redelivery");
    assert_eq!(sink.received(), 1, "published exactly once despite the redelivery");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM communication.messages WHERE channel='whatsapp' AND external_id=$1")
        .bind(&ext).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1, "only one row for the provider message");
}

// CGC-3 — routing reuses an open thread for the same party+channel (two inbound messages, one thread).
#[tokio::test]
async fn cgc3_routing_reuses_thread_for_party() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let party = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();

    // Distinct external_ref so routing falls to the party rule.
    let mut a = inbound(company, &format!("m-{}", Uuid::new_v4()), Some(party)); a.external_ref = None;
    let mut b = inbound(company, &format!("m-{}", Uuid::new_v4()), Some(party)); b.external_ref = None;
    let first = svc.receive_inbound(a, &sink).await.unwrap();
    let second = svc.receive_inbound(b, &sink).await.unwrap();

    assert_eq!(first.thread_id, second.thread_id, "same party+channel → one open thread");
    assert_eq!(sink.received(), 2, "two distinct messages published");
}

// CGC-4 — outbound send goes through the channel and lands 'sent' with the provider id; a delivery receipt
// then marks it 'delivered' and publishes MessageDelivered exactly once.
#[tokio::test]
async fn cgc4_outbound_send_then_delivery_receipt() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let channel = FakeChannel::new();
    let sink = CapturingSink::new();

    let thread = svc.open_thread(company, "whatsapp", Some(Uuid::new_v4()), None).await.unwrap();
    let msg = svc.send_outbound(thread, "+628123".into(), "Pesanan Anda dikirim".into(), &channel, &sink).await.unwrap();
    assert_eq!(channel.count(), 1, "one send reached the provider");

    let (status, ext): (String, Option<String>) = sqlx::query_as(
        "SELECT status::text, external_id FROM communication.messages WHERE id=$1")
        .bind(msg).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "sent");
    let ext = ext.expect("provider id assigned");

    // Delivery receipt (idempotent) — first flips to delivered + publishes; a redelivered receipt is a no-op.
    svc.mark_delivered("whatsapp", company, &ext, &sink).await.unwrap();
    svc.mark_delivered("whatsapp", company, &ext, &sink).await.unwrap();
    let status2: String = sqlx::query_scalar("SELECT status::text FROM communication.messages WHERE id=$1")
        .bind(msg).fetch_one(&pool).await.unwrap();
    assert_eq!(status2, "delivered");
    assert_eq!(sink.delivered(), 1, "MessageDelivered published exactly once");
    let _ = LoggingSink;
}

// CGC-5 — a follow-up reply carries the thread's LINKED subject so the consumer appends, not duplicates
// (completeness council 2026-07-08). Msg #1 opens a thread; the consumer links it to issue X; msg #2
// (a plain reply with no subject hint) routes onto the same thread and its MessageReceived names issue X.
#[tokio::test]
async fn cgc5_followup_carries_linked_subject() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let party = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let sink = CapturingSink::new();
    let issue_x = Uuid::new_v4();

    // Msg #1 → new thread; consumer opens issue X and links the thread.
    let mut m1 = inbound(company, &format!("m-{}", Uuid::new_v4()), Some(party));
    m1.external_ref = Some("conv-append".into());
    let first = svc.receive_inbound(m1, &sink).await.unwrap();
    svc.link_thread(first.thread_id, "issue", issue_x).await.unwrap();

    // Msg #2 → a plain reply, NO subject hint, same conversation.
    let mut m2 = inbound(company, &format!("m-{}", Uuid::new_v4()), Some(party));
    m2.external_ref = Some("conv-append".into());
    m2.subject_type = None; m2.subject_id = None;
    let second = svc.receive_inbound(m2, &sink).await.unwrap();

    assert_eq!(first.thread_id, second.thread_id, "reply routes onto the same thread");
    let ev = sink.last_received();
    assert_eq!(ev.subject_type.as_deref(), Some("issue"), "the reply names the linked work item");
    assert_eq!(ev.subject_id, Some(issue_x), "so the consumer appends to issue X, not a duplicate");
}
