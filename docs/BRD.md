# backbone-communication — BRD

## Documents
Thread (a conversation with a party over one channel) · Message (inbound/outbound). Own Postgres schema
`communication`. Posts **no GL**. Publishes inbound events over the event bus.

## Business rules

**BR-1 (inbound dedup — the invariant).** `receive_inbound` records a message and publishes
`MessageReceived` **exactly once per (channel, external_id)**. Provider webhooks are at-least-once; a
redelivery must not create a second message nor re-route. A provider `external_id` is required (no dedup
key otherwise). The `MessageReceived` event is **staged into the transactional outbox in the SAME
transaction as the message insert**, so the routing event and the message row commit atomically — a crash
between commit and publish can never lose the event (maturity council 2026-07-08).

**BR-2 (routing).** An inbound message routes onto an OPEN thread: by provider conversation handle
(`external_ref`), else by (company, channel, party), else a new thread is opened. The thread links to the
party and — once a consumer decides what it concerns — to a lead/issue/order via `link_thread`
(`subject_type` + `subject_id`, a polymorphic logical reference).

**BR-3 (outbound send).** `send_outbound` records a `queued` message and hands it to the `ChannelPort`
(WhatsApp/email/SMS). On acceptance it becomes `sent` with the provider's `external_id`; on rejection it
becomes `failed` with the reason (the failure is **recorded, not swallowed**) and `MessageFailed` is
published. A closed thread refuses sends.

**BR-4 (delivery receipts).** `mark_delivered(channel, external_id)` transitions a `sent` message to
`delivered` **once** (gated on status) and publishes `MessageDelivered`; a redelivered receipt is a no-op.

**BR-5 (thread lifecycle).** A thread is `open` until `close_thread` archives it (no further sends).

## Events
`MessageReceived` (message_id, thread_id, company_id, channel, party_id?, subject_type?, subject_id?,
address_from?, body) — the inbound-routing signal CRM/support consume. Its `subject_type/subject_id` are
**hydrated from the thread's persisted `link_thread` value** (webhook hint is a fallback for the first
message), so a follow-up reply names the already-open work item and a consumer appends rather than
duplicating (completeness council 2026-07-08). `MessageDelivered`, `MessageFailed` — outbound fate.

## Deferred (with reason)
Concrete provider adapters (wired behind `ChannelPort`), campaign automation / IVR / social (cut for
SMB), telephony (→ backbone-telephony), templated outbound fan-out (→ backbone-notification).
