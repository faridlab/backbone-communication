//! The hand-authored communication write path (user-owned; survives regen).
//!
//! The channel gateway: record inbound messages **idempotently** (provider webhooks are at-least-once, so
//! a redelivery must not create a second message nor re-publish `MessageReceived`), route them onto a
//! thread, and send outbound messages through the `ChannelPort`. Posts NO GL. Threads link to the party
//! and (optionally) the lead/issue/order they concern via a polymorphic logical reference.

use backbone_orm::company_scope;
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    MessageRepository, NewInboundMessageRow, NewOutboundMessageRow, NewRoutedThreadRow, NewThreadRow,
    ThreadRepository,
};

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
    messages: MessageRepository,
    threads: ThreadRepository,
}

impl CommunicationWriteService {
    pub fn new(pool: PgPool) -> Self {
        let messages = MessageRepository::new(pool.clone());
        let threads = ThreadRepository::new(pool.clone());
        Self { pool, messages, threads }
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
        if let Some(row) = company_scope::with_company_scope(
            Some(m.company_id),
            self.messages.find_by_external_id(&self.pool, &m.channel, &m.external_id),
        ).await?
        {
            return Ok(InboundOutcome {
                thread_id: row.thread_id, message_id: row.id, duplicate: true,
            });
        }

        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, m.company_id).await?;
        let routed = self.resolve_or_open_thread(&mut tx, &m).await?;
        let thread_id = routed.id;

        // Insert gated on the unique (channel, external_id) — if we lost a race, the row is absent.
        let inserted = self.messages.claim_inbound(&mut tx, &NewInboundMessageRow {
            id: Uuid::new_v4(),
            thread_id,
            company_id: m.company_id,
            channel: &m.channel,
            external_id: &m.external_id,
            address_from: m.address_from.as_deref(),
            address_to: m.address_to.as_deref(),
            body: &m.body,
        }).await?;

        let Some(message_id) = inserted else {
            // Lost the race — discard the thread we may have opened; return the winner's message.
            tx.rollback().await?;
            let row = company_scope::with_company_scope(
                Some(m.company_id),
                self.messages.fetch_by_external_id(&self.pool, &m.channel, &m.external_id),
            ).await?;
            return Ok(InboundOutcome {
                thread_id: row.thread_id, message_id: row.id, duplicate: true,
            });
        };

        self.threads.touch_last_message_on(&mut tx, thread_id).await?;

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
            "MessageReceived", "Message", message_id.to_string(), m.company_id,
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
        company_scope::with_company_scope(
            Some(company_id),
            self.threads.insert_thread(&self.pool, &NewThreadRow {
                id,
                company_id,
                channel,
                party_id,
                external_ref: external_ref.as_deref(),
            }),
        ).await?;
        Ok(id)
    }

    /// Attach a thread to the business object it concerns (a lead, an issue, an order) after routing.
    pub async fn link_thread(&self, thread_id: Uuid, subject_type: &str, subject_id: Uuid) -> Result<(), CommError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by thread id alone, with no company to
        // scope from. The update rides the REQUEST-dedicated connection (established by
        // `company_auth`), which carries the caller's `app.company_id`; RLS fences it so another
        // company's thread simply isn't matched and this reports NotFound.
        let n = self.threads.set_subject(&self.pool, thread_id, subject_type, subject_id).await?;
        if n != 1 {
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
        let thread = self
            .threads
            .fetch_for_send(&self.pool, thread_id)
            .await?
            .ok_or(CommError::NotFound("thread"))?;
        if thread.status != "open" {
            return Err(CommError::InvalidState("thread is closed"));
        }
        let company_id = thread.company_id;
        let channel = thread.channel;

        let message_id = Uuid::new_v4();
        company_scope::with_company_scope(
            Some(company_id),
            self.messages.insert_outbound(&self.pool, &NewOutboundMessageRow {
                id: message_id,
                thread_id,
                company_id,
                channel: &channel,
                address_to: &address_to,
                body: &body,
            }),
        ).await?;

        match port.send(&OutboundSend { channel: channel.clone(), to: address_to, body }).await {
            Ok(ack) => {
                company_scope::with_company_scope(
                    Some(company_id),
                    self.messages.mark_sent(&self.pool, message_id, &ack.external_id),
                ).await?;
                company_scope::with_company_scope(
                    Some(company_id),
                    self.threads.touch_last_message(&self.pool, thread_id),
                ).await?;
                Ok(message_id)
            }
            Err(rej) => {
                company_scope::with_company_scope(
                    Some(company_id),
                    self.messages.mark_failed(&self.pool, message_id, &rej.message),
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
        let moved = self.messages.mark_delivered(&self.pool, channel, external_id).await?;
        if let Some(message_id) = moved {
            events.publish(&CommunicationEvent::MessageDelivered { message_id, external_id: external_id.to_string() });
        }
        Ok(())
    }

    /// Close a thread (no further sends).
    pub async fn close_thread(&self, thread_id: Uuid) -> Result<(), CommError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by thread id alone; the update rides the
        // request-dedicated connection, whose `app.company_id` fences it.
        let n = self.threads.mark_closed(&self.pool, thread_id).await?;
        if n != 1 {
            return Err(CommError::InvalidState("thread is not open"));
        }
        Ok(())
    }

    /// Route an inbound message onto a thread: reuse an open thread by provider conversation handle, else
    /// by party, else open a new one. Returns the thread's PERSISTED subject link so the routing event can
    /// name the work item a follow-up belongs to (a consumer appends instead of opening a duplicate).
    ///
    /// Runs entirely on the CALLER'S tx — the thread it opens must roll back with the message insert when
    /// the dedup claim loses its race. The caller has already bound the webhook's company on that tx.
    async fn resolve_or_open_thread(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        m: &InboundMessage,
    ) -> Result<RoutedThread, CommError> {
        // Two routing keys, tried in order: provider conversation handle, then party.
        if let Some(ext) = m.external_ref.as_ref() {
            if let Some(row) = self
                .threads
                .find_open_by_external_ref(&mut **tx, m.company_id, &m.channel, ext)
                .await?
            {
                return Ok(RoutedThread {
                    id: row.id, subject_type: row.subject_type, subject_id: row.subject_id,
                });
            }
        }
        if let Some(party) = m.party_id {
            if let Some(row) = self
                .threads
                .find_open_by_party(&mut **tx, m.company_id, &m.channel, party)
                .await?
            {
                return Ok(RoutedThread {
                    id: row.id, subject_type: row.subject_type, subject_id: row.subject_id,
                });
            }
        }
        // No open thread — open one carrying the webhook's subject hint (if any).
        let id = Uuid::new_v4();
        self.threads.open_routed_thread(&mut **tx, &NewRoutedThreadRow {
            id,
            company_id: m.company_id,
            channel: &m.channel,
            party_id: m.party_id,
            external_ref: m.external_ref.as_deref(),
            subject_type: m.subject_type.as_deref(),
            subject_id: m.subject_id,
        }).await?;
        Ok(RoutedThread { id, subject_type: m.subject_type.clone(), subject_id: m.subject_id })
    }
}

/// A routed thread and the subject it is currently linked to (the persisted `link_thread` value, which a
/// follow-up webhook does not carry).
struct RoutedThread {
    id: Uuid,
    subject_type: Option<String>,
    subject_id: Option<Uuid>,
}

