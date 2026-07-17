//! The hand-authored communication write path (user-owned; survives regen).
//!
//! The channel gateway: record inbound messages **idempotently** (provider webhooks are at-least-once, so
//! a redelivery must not create a second message nor re-publish `MessageReceived`), route them onto a
//! thread, and send outbound messages through the `ChannelPort`. Posts NO GL. Threads link to the party
//! and (optionally) the lead/issue/order they concern via a polymorphic logical reference.

use backbone_orm::company_scope;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::communication_events::*;
use super::communication_ports::*;

#[derive(Debug, thiserror::Error)]
pub enum CommError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("channel rejected: {0}")]
    ChannelRejected(String),
}

/// An inbound message as delivered by a channel provider (a webhook).
pub struct InboundMessage {
    pub company_id: Uuid,
    pub channel: String, // whatsapp | email | sms
    /// The provider's message id — the dedup key. Required for inbound (webhooks carry one).
    pub external_id: String,
    pub address_from: Option<String>,
    pub address_to: Option<String>,
    pub body: String,
    pub party_id: Option<Uuid>,
    /// The provider conversation handle — used to route onto an existing thread when present.
    pub external_ref: Option<String>,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InboundOutcome {
    pub thread_id: Uuid,
    pub message_id: Uuid,
    /// True when this webhook was a redelivery — nothing was written and no event was published.
    pub duplicate: bool,
}

pub struct CommunicationWriteService {
    pool: PgPool,
}

impl CommunicationWriteService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record an inbound message and publish `MessageReceived` — **exactly once per (channel,
    /// external_id)**. A redelivered webhook returns the original outcome with `duplicate=true`, writes
    /// nothing, and does NOT re-publish. Routes onto an existing open thread (by `external_ref`, else by
    /// party) or opens one.
    pub async fn receive_inbound(
        &self,
        m: InboundMessage,
        events: &dyn CommunicationEventSink,
    ) -> Result<InboundOutcome, CommError> {
        if m.external_id.trim().is_empty() {
            return Err(CommError::Invalid("inbound message needs a provider external_id".into()));
        }
        if m.body.is_empty() {
            return Err(CommError::Invalid("inbound message needs a body".into()));
        }
        // Fast path: already seen this provider message → return the original, publish nothing.
        // RLS scope (ADR-0008), DTO-company pattern: the webhook payload names the company, so every
        // read/write below is fenced to it explicitly (correct for non-request callers too).
        let dedup_q = sqlx::query(
            "SELECT id, thread_id FROM communication.messages WHERE channel=$1::channel AND external_id=$2")
            .bind(&m.channel).bind(&m.external_id);
        if let Some(row) = company_scope::with_company_scope(
            Some(m.company_id),
            company_scope::fetch_optional_row_scoped(&self.pool, dedup_q),
        ).await?
        {
            return Ok(InboundOutcome {
                thread_id: row.get("thread_id"), message_id: row.get("id"), duplicate: true,
            });
        }

        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, m.company_id).await?;
        let routed = resolve_or_open_thread(&mut tx, &m).await?;
        let thread_id = routed.id;

        // Insert gated on the unique (channel, external_id) — if we lost a race, the row is absent.
        let inserted: Option<Uuid> = sqlx::query_scalar(
            r#"INSERT INTO communication.messages
                 (id, thread_id, company_id, direction, channel, external_id, address_from, address_to,
                  body, status, occurred_at)
               VALUES ($1,$2,$3,'inbound'::direction,$4::channel,$5,$6,$7,$8,'received'::message_status, now())
               ON CONFLICT (channel, external_id) DO NOTHING
               RETURNING id"#,
        )
        .bind(Uuid::new_v4()).bind(thread_id).bind(m.company_id).bind(&m.channel).bind(&m.external_id)
        .bind(&m.address_from).bind(&m.address_to).bind(&m.body)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(message_id) = inserted else {
            // Lost the race — discard the thread we may have opened; return the winner's message.
            tx.rollback().await?;
            let winner_q = sqlx::query(
                "SELECT id, thread_id FROM communication.messages WHERE channel=$1::channel AND external_id=$2")
                .bind(&m.channel).bind(&m.external_id);
            let row = company_scope::with_company_scope(
                Some(m.company_id),
                company_scope::fetch_one_row_scoped(&self.pool, winner_q),
            ).await?;
            return Ok(InboundOutcome {
                thread_id: row.get("thread_id"), message_id: row.get("id"), duplicate: true,
            });
        };

        sqlx::query("UPDATE communication.threads SET last_message_at = now() WHERE id=$1")
            .bind(thread_id)
            .execute(&mut *tx)
            .await?;

        // Stage the routing event in the SAME tx as the message insert, so it commits atomically. This is
        // what makes routing exactly-once and DURABLE — a crash between commit and the in-proc publish
        // below can no longer lose MessageReceived; the relay drains the outbox at-least-once and consumers
        // dedup via the inbox (maturity council 2026-07-08).
        // The event names what the thread concerns from its PERSISTED link (set by a prior link_thread),
        // falling back to the webhook hint only for a thread's first message — so a follow-up reply routes
        // to the already-open work item instead of spawning a duplicate (completeness council 2026-07-08).
        let received = MessageReceived {
            message_id, thread_id, company_id: m.company_id, channel: m.channel.clone(),
            party_id: m.party_id,
            subject_type: routed.subject_type.or_else(|| m.subject_type.clone()),
            subject_id: routed.subject_id.or(m.subject_id),
            address_from: m.address_from.clone(), body: m.body.clone(),
        };
        let record = backbone_outbox::OutboxRecord::new(
            "MessageReceived", "Message", message_id.to_string(),
            serde_json::to_value(&received).map_err(|e| CommError::Invalid(e.to_string()))?,
            chrono::Utc::now(),
        );
        backbone_outbox::outbox::stage(&mut *tx, "communication", &record)
            .await
            .map_err(|e| CommError::Invalid(format!("outbox stage: {e}")))?;

        tx.commit().await?;

        // In-proc convenience delivery (immediate); the outbox above is the durable backstop.
        events.publish(&CommunicationEvent::MessageReceived(received));
        Ok(InboundOutcome { thread_id, message_id, duplicate: false })
    }

    /// Open a thread explicitly (outbound-initiated conversation).
    pub async fn open_thread(
        &self, company_id: Uuid, channel: &str, party_id: Option<Uuid>, external_ref: Option<String>,
    ) -> Result<Uuid, CommError> {
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008), param-company pattern: the company is an argument — fence to it.
        let insert_q = sqlx::query(
            r#"INSERT INTO communication.threads (id, company_id, channel, party_id, external_ref, status)
               VALUES ($1,$2,$3::channel,$4,$5,'open'::thread_status)"#,
        )
        .bind(id).bind(company_id).bind(channel).bind(party_id).bind(&external_ref);
        company_scope::with_company_scope(
            Some(company_id),
            company_scope::execute_scoped(&self.pool, insert_q),
        ).await?;
        Ok(id)
    }

    /// Attach a thread to the business object it concerns (a lead, an issue, an order) after routing.
    pub async fn link_thread(&self, thread_id: Uuid, subject_type: &str, subject_id: Uuid) -> Result<(), CommError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by thread id alone, with no company to
        // scope from. The update rides the REQUEST-dedicated connection (established by
        // `company_auth`), which carries the caller's `app.company_id`; RLS fences it so another
        // company's thread simply isn't matched and this reports NotFound.
        let link_q = sqlx::query(
            r#"UPDATE communication.threads SET subject_type=$2, subject_id=$3
               WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(thread_id).bind(subject_type).bind(subject_id);
        let n = company_scope::execute_scoped(&self.pool, link_q).await?;
        if n.rows_affected() != 1 {
            return Err(CommError::NotFound("thread"));
        }
        Ok(())
    }

    /// Send a message out through the channel provider. Records a `queued` message, drives the
    /// `ChannelPort`, then marks it `sent` (+ provider id) or `failed`. Publishes `MessageFailed` on
    /// rejection so a consumer can react.
    pub async fn send_outbound(
        &self,
        thread_id: Uuid,
        address_to: String,
        body: String,
        port: &dyn ChannelPort,
        events: &dyn CommunicationEventSink,
    ) -> Result<Uuid, CommError> {
        if body.is_empty() {
            return Err(CommError::Invalid("outbound message needs a body".into()));
        }
        // RLS scope (ADR-0008), ID-only pattern: identified by thread id alone — no company argument to
        // scope from up front. This read rides the REQUEST-dedicated connection, whose `app.company_id`
        // fences it. Having read the thread, we bind its company explicitly on the writes below.
        let thread_q = sqlx::query(
            r#"SELECT company_id, channel::text AS channel, status::text AS status
               FROM communication.threads WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(thread_id);
        let thread = company_scope::fetch_optional_row_scoped(&self.pool, thread_q)
            .await?
            .ok_or(CommError::NotFound("thread"))?;
        if thread.get::<String, _>("status") != "open" {
            return Err(CommError::InvalidState("thread is closed"));
        }
        let company_id: Uuid = thread.get("company_id");
        let channel: String = thread.get("channel");

        let message_id = Uuid::new_v4();
        let queue_q = sqlx::query(
            r#"INSERT INTO communication.messages
                 (id, thread_id, company_id, direction, channel, address_to, body, status, occurred_at)
               VALUES ($1,$2,$3,'outbound'::direction,$4::channel,$5,$6,'queued'::message_status, now())"#,
        )
        .bind(message_id).bind(thread_id).bind(company_id).bind(&channel).bind(&address_to).bind(&body);
        company_scope::with_company_scope(
            Some(company_id),
            company_scope::execute_scoped(&self.pool, queue_q),
        ).await?;

        match port.send(&OutboundSend { channel: channel.clone(), to: address_to, body }).await {
            Ok(ack) => {
                let sent_q = sqlx::query(
                    r#"UPDATE communication.messages
                       SET status='sent'::message_status, external_id=$2 WHERE id=$1"#,
                )
                .bind(message_id).bind(&ack.external_id);
                company_scope::with_company_scope(
                    Some(company_id),
                    company_scope::execute_scoped(&self.pool, sent_q),
                ).await?;
                let touch_q = sqlx::query("UPDATE communication.threads SET last_message_at = now() WHERE id=$1")
                    .bind(thread_id);
                company_scope::with_company_scope(
                    Some(company_id),
                    company_scope::execute_scoped(&self.pool, touch_q),
                ).await?;
                Ok(message_id)
            }
            Err(rej) => {
                let failed_q = sqlx::query(
                    r#"UPDATE communication.messages
                       SET status='failed'::message_status, failure_reason=$2 WHERE id=$1"#,
                )
                .bind(message_id).bind(&rej.message);
                company_scope::with_company_scope(
                    Some(company_id),
                    company_scope::execute_scoped(&self.pool, failed_q),
                ).await?;
                events.publish(&CommunicationEvent::MessageFailed { message_id, reason: rej.code.clone() });
                Err(CommError::ChannelRejected(rej.code))
            }
        }
    }

    /// Record a provider delivery receipt for an outbound message (idempotent — a redelivered receipt is
    /// a no-op). Publishes `MessageDelivered` on the first transition.
    pub async fn mark_delivered(
        &self, channel: &str, external_id: &str, events: &dyn CommunicationEventSink,
    ) -> Result<(), CommError> {
        // RLS scope (ADR-0008), ID-only pattern: a delivery receipt names only (channel, external_id) —
        // no company. Under HTTP the request-dedicated connection carries the scope. When driven by an
        // EVENT (a provider-callback consumer), the CALLER must wrap this in
        // `with_company_scope(Some(event.company_id))` — otherwise the update fails closed.
        let deliver_q = sqlx::query(
            r#"UPDATE communication.messages SET status='delivered'::message_status
               WHERE channel=$1::channel AND external_id=$2 AND status='sent'::message_status
               RETURNING id"#,
        )
        .bind(channel).bind(external_id);
        let moved = company_scope::fetch_optional_row_scoped(&self.pool, deliver_q).await?;
        if let Some(row) = moved {
            let message_id: Uuid = row.get("id");
            events.publish(&CommunicationEvent::MessageDelivered { message_id, external_id: external_id.to_string() });
        }
        Ok(())
    }

    /// Close a thread (no further sends).
    pub async fn close_thread(&self, thread_id: Uuid) -> Result<(), CommError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by thread id alone; the update rides the
        // request-dedicated connection, whose `app.company_id` fences it.
        let close_q = sqlx::query(
            r#"UPDATE communication.threads SET status='closed'::thread_status
               WHERE id=$1 AND status='open'::thread_status"#,
        )
        .bind(thread_id);
        let n = company_scope::execute_scoped(&self.pool, close_q).await?;
        if n.rows_affected() != 1 {
            return Err(CommError::InvalidState("thread is not open"));
        }
        Ok(())
    }
}

/// A routed thread and the subject it is currently linked to (the persisted `link_thread` value, which a
/// follow-up webhook does not carry).
struct RoutedThread {
    id: Uuid,
    subject_type: Option<String>,
    subject_id: Option<Uuid>,
}

/// Route an inbound message onto a thread: reuse an open thread by provider conversation handle, else by
/// party, else open a new one. Returns the thread's PERSISTED subject link so the routing event can name
/// the work item a follow-up belongs to (a consumer appends instead of opening a duplicate).
async fn resolve_or_open_thread(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    m: &InboundMessage,
) -> Result<RoutedThread, CommError> {
    for (col, val) in [("external_ref", m.external_ref.as_ref()), ("party_id", None)] {
        // Two routing keys, tried in order: provider conversation handle, then party.
        let found = match col {
            "external_ref" => {
                let Some(ext) = val else { continue };
                sqlx::query(
                    r#"SELECT id, subject_type, subject_id FROM communication.threads
                       WHERE company_id=$1 AND channel=$2::channel AND external_ref=$3
                         AND status='open'::thread_status AND (metadata->>'deleted_at') IS NULL
                       ORDER BY last_message_at DESC NULLS LAST LIMIT 1"#,
                )
                .bind(m.company_id).bind(&m.channel).bind(ext)
                .fetch_optional(&mut **tx).await?
            }
            _ => {
                let Some(party) = m.party_id else { continue };
                sqlx::query(
                    r#"SELECT id, subject_type, subject_id FROM communication.threads
                       WHERE company_id=$1 AND channel=$2::channel AND party_id=$3
                         AND status='open'::thread_status AND (metadata->>'deleted_at') IS NULL
                       ORDER BY last_message_at DESC NULLS LAST LIMIT 1"#,
                )
                .bind(m.company_id).bind(&m.channel).bind(party)
                .fetch_optional(&mut **tx).await?
            }
        };
        if let Some(row) = found {
            return Ok(RoutedThread {
                id: row.get("id"), subject_type: row.get("subject_type"), subject_id: row.get("subject_id"),
            });
        }
    }
    // No open thread — open one carrying the webhook's subject hint (if any).
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO communication.threads
             (id, company_id, channel, party_id, external_ref, subject_type, subject_id, status)
           VALUES ($1,$2,$3::channel,$4,$5,$6,$7,'open'::thread_status)"#,
    )
    .bind(id).bind(m.company_id).bind(&m.channel).bind(m.party_id).bind(&m.external_ref)
    .bind(&m.subject_type).bind(m.subject_id)
    .execute(&mut **tx)
    .await?;
    Ok(RoutedThread { id, subject_type: m.subject_type.clone(), subject_id: m.subject_id })
}
