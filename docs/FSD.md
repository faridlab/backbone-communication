# backbone-communication — FSD

## Entities
Thread (`company_id`, `channel`, `party_id?` logical FK, `subject_type?`/`subject_id?` polymorphic logical
ref, `external_ref?`, `status`, `last_message_at?`) · Message (`thread_id` FK, `company_id`, `direction`,
`channel`, `external_id?`, `address_from?`/`address_to?`, `body`, `status`, `failure_reason?`,
`occurred_at`). Enums: Channel {whatsapp, email, sms}, Direction {inbound, outbound}, ThreadStatus {open,
closed}, MessageStatus {received, queued, sent, delivered, failed}. Unique `(channel, external_id)` — the
inbound-dedup + send-once guard (NULLs distinct, so outbound rows without a provider id yet never collide).

## Write path (`CommunicationWriteService`, hand-authored, user-owned)
- `receive_inbound(InboundMessage, &dyn CommunicationEventSink)` → dedup on (channel, external_id); route
  onto a thread; insert + **stage `MessageReceived` in the same tx (transactional outbox)** + publish;
  returns `InboundOutcome {thread_id, message_id, duplicate}`. Exactly-once and durable.
- `open_thread` / `link_thread(subject_type, subject_id)` — open a thread, attach it to a lead/issue/order
- `send_outbound(thread, to, body, &dyn ChannelPort, &dyn CommunicationEventSink)` → queued → sent(+provider
  id)/failed; publishes `MessageFailed` on rejection
- `mark_delivered(channel, external_id, sink)` → gated `sent → delivered`, publishes `MessageDelivered` once
- `close_thread`

Errors: `CommError {Db, NotFound, Invalid, InvalidState, ChannelRejected}`.

## Seams (ports — zero normal Cargo edge to any domain module)
- **Outbound → channel provider:** `ChannelPort` (WhatsApp Business API / email / SMS) — a composing service
  wires the real adapter; the module ships the port, never an SDK.
- **Inbound → event bus (proven, CSEAM-1):** `MessageReceived` is staged to the outbox + published to a
  `CommunicationEventSink`; a consumer (proven: REAL backbone-support `raise_issue`) opens a ticket from the
  event alone. Routing is an event, not a driven call.
- **Durability:** the transactional outbox (`backbone-outbox`, a framework crate) — the event commits with
  the message.

## Test oracle
`communication_golden_cases` (5: CGC-1 inbound recorded+published, CGC-2 redelivery idempotent, CGC-3
routing reuses a party thread, CGC-4 outbound send + delivery receipt, CGC-5 follow-up reply carries the
thread's linked subject so a consumer appends not duplicates),
`integrity_probes` (6: CIP-1 inbound needs external_id, CIP-2 closed thread refuses send, CIP-3 channel
rejection recorded, CIP-4 delivery receipt safe/idempotent, CIP-5 routing by conversation handle, CIP-6
routing event durable via outbox),
`communication_support_seam` (1: CSEAM-1 inbound message opens a REAL support ticket) + §5 round-trip.
**12 tests.**

> The generated `integration_tests.rs` hits an external HTTP server and is environmental scaffolding, not
> part of this module's correctness gate.
