//! The inbound-routing seam against the REAL backbone-support module. An inbound WhatsApp message routes
//! through `MessageReceived`; a consumer (here, the test acting as the composing service) opens a REAL
//! support ticket from the event alone. Proves the event carries what a consumer needs. ZERO normal Cargo
//! edge to support — support is a dev-dependency only, and routing is an event, not a driven call.

mod common;
use common::*;

use backbone_support::application::service::support_write_service::{NewIssue, SupportWriteService};
use backbone_communication::application::service::communication_write_service::*;
use uuid::Uuid;

// CSEAM-1 — an inbound WhatsApp message opens a REAL support ticket carrying the party + body, driven
// only by the fields on MessageReceived.
#[tokio::test]
async fn cseam1_inbound_message_opens_real_support_ticket() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let party = Uuid::new_v4();
    let svc = CommunicationWriteService::new(pool.clone());
    let support = SupportWriteService::new(pool.clone());
    let sink = CapturingSink::new();

    let m = InboundMessage {
        company_id: company, channel: "whatsapp".into(), external_id: format!("m-{}", Uuid::new_v4()),
        address_from: Some("+628123".into()), address_to: Some("+628999".into()),
        body: "Barang saya rusak, tolong bantu".into(), party_id: Some(party),
        external_ref: Some("wa-conv-9".into()), subject_type: None, subject_id: None,
    };
    svc.receive_inbound(m, &sink).await.unwrap();

    // The composing service consumes MessageReceived and opens a REAL support ticket from it alone.
    let ev = sink.last_received();
    let issue_id = support.raise_issue(NewIssue {
        company_id: ev.company_id,
        customer_id: ev.party_id,
        subject: format!("WhatsApp: {}", ev.body.chars().take(60).collect::<String>()),
        description: Some(ev.body.clone()),
        priority: "medium".into(),
        sla_id: None,
    }, chrono::Utc::now()).await.expect("real support raises the ticket");

    // A REAL support issue exists, tied to the party, carrying the message body.
    let (customer, desc, status): (Option<Uuid>, Option<String>, String) = sqlx::query_as(
        "SELECT customer_id, description, status::text FROM support.issues WHERE id=$1")
        .bind(issue_id).fetch_one(&pool).await.unwrap();
    assert_eq!(customer, Some(party), "ticket tied to the messaging party");
    assert_eq!(desc.as_deref(), Some("Barang saya rusak, tolong bantu"), "carries the message body");
    assert_eq!(status, "open");
}
