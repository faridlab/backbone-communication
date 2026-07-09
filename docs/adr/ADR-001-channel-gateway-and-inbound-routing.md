# ADR-001 — The channel gateway, inbound dedup, and durable event-driven routing

Status: accepted · 2026-07-08 · Channels pillar (Tier 5; posts no GL)

## Context
The Indonesia SMB lives on WhatsApp. CRM parked "inbound WhatsApp → lead", support parked "email/WhatsApp
→ ticket", and every module needs to notify. That is one shared bounded context — a **channel gateway** —
not per-module glue. This module records inbound/outbound messages on threads and turns an inbound message
into a domain event consumers route on.

## Decision
1. **Inbound is idempotent on (channel, external_id) — a DB invariant.** Provider webhooks are
   at-least-once; the unique index + `INSERT … ON CONFLICT DO NOTHING RETURNING id` guarantees one message
   per provider id regardless of interleaving (the fast-path SELECT is only an optimization). A redelivery
   returns `duplicate=true` and writes nothing.
2. **Routing is an EVENT, not a driven call.** `receive_inbound` publishes `MessageReceived`; CRM (capture a
   lead) and support (open/append an issue) subscribe over the event bus. The module has zero Cargo edge to
   any consumer — proven by CSEAM-1 driving REAL backbone-support from the event alone.
3. **The routing event is DURABLE — staged in the transactional outbox in the same tx as the message.** A
   dual write (commit, then publish) would lose the event on a crash between the two, and the dedup would
   swallow the provider's redelivery — the event would be lost forever. Staging `MessageReceived` via
   `backbone_outbox::outbox::stage` inside the insert tx makes the event a committed fact; a relay delivers
   at-least-once, consumers dedup via the inbox (maturity council 2026-07-08).
4. **The outside world is a port.** Outbound sends go through `ChannelPort` (WhatsApp/email/SMS); the module
   ships the port, a composing service wires the provider SDK. No transport dependency leaks in.
5. **Posts no GL.** Threads link to a party and (once a consumer decides) to a lead/issue/order via a
   polymorphic logical reference (`subject_type` + `subject_id`).

## Consequences
- Turn communication off and no message routes; it is the one place inbound channel traffic becomes domain
  events. The event bus decouples it from every consumer.
- Proven end-to-end against REAL backbone-support and durable across a lost publish (CIP-6); survives regen
  (§5).

## Parking lot (each with a gate)
- **Follow-up replies spawned duplicate work items** — FIXED (completeness council 2026-07-08):
  `MessageReceived` was built from the raw webhook, so a reply on an already-linked thread carried
  `subject=None` and a consumer opened a duplicate ticket; fixed by hydrating the event's subject from the
  thread's persisted `link_thread` value (thread wins, webhook hint fallback) so a follow-up names the
  open work item (CGC-5, proven-by-revert).
- **Routing event was not durable** — FIXED (maturity council 2026-07-08): the commit→publish dual write
  dropped `MessageReceived` on a crash-in-window while the dedup swallowed the redelivery; fixed by staging
  the event in the transactional outbox in the same tx (CIP-6, proven-by-revert).
- **Outbound double-send on retry** — `send_outbound` drives the provider outside a tx with no idempotency
  key; a crash after acceptance strands a `queued` row and a retry re-sends. Gate: a `ChannelPort`
  idempotency key from `message_id` + a queued-reconciliation sweep.
- **Duplicate open threads for concurrent first inbounds from one party** — cosmetic thread split, no lost
  message. Gate: a partial-unique (company, channel, party) on open threads.
- **The event sink can't report failure** — `publish(&self) -> ()`; the outbox makes durability independent
  of it. Gate: a `Result`-returning relay-backed sink.
- **Concrete channel adapters + campaign automation** — deferred (PRD non-goals).
