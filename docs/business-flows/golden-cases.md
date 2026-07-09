# backbone-communication — business flows & golden cases

## Flow: inbound webhook → dedup → route → publish (durably)
```
receive_inbound (provider webhook, at-least-once)
   │
   ▼  dedup on (channel, external_id) — redelivery → duplicate=true, write nothing, publish nothing
   │
   ▼  route onto a thread (external_ref → party → new) + INSERT message (received)
   │
   ▼  STAGE MessageReceived in the outbox — SAME tx as the insert (durable) → commit
   │
   ├▶ publish MessageReceived (in-proc, immediate) + relay drains the outbox (at-least-once)
   │
   └▶ CRM captures a lead · support opens/appends an issue (consumer dedups via inbox)
```
Outbound: `send_outbound` → queued → ChannelPort → sent(+provider id)/failed; `mark_delivered` gates
`sent → delivered` once. Posts NO GL.

## Golden cases (`tests/communication_golden_cases.rs`)
- **CGC-1 — inbound recorded + published.** One inbound message → an inbound/received row + exactly one
  `MessageReceived` carrying the body.
- **CGC-2 — redelivery idempotent.** The same (channel, external_id) twice → one row, one publish, same
  ids, `duplicate=true`, no spurious thread.
- **CGC-3 — routing reuses a party thread.** Two inbound messages from one party+channel → one open thread,
  two published events.
- **CGC-4 — outbound send + delivery receipt.** `send_outbound` → provider gets one send, row is `sent`
  with the provider id; `mark_delivered` flips to `delivered` and publishes once (a redelivered receipt is
  a no-op).
- **CGC-5 — follow-up carries the linked subject.** Msg #1 opens a thread → consumer links it to issue X →
  msg #2 (a plain reply, no hint) routes onto the same thread and its `MessageReceived` names issue X, so
  the consumer appends instead of opening a duplicate ticket.

## Integrity probes (`tests/integrity_probes.rs`)
- **CIP-1 — inbound needs external_id.** No provider id → refused (no dedup key).
- **CIP-2 — closed thread refuses send.**
- **CIP-3 — channel rejection recorded.** A provider rejection persists a `failed` row + reason and
  surfaces the error (not swallowed).
- **CIP-4 — delivery receipt safe.** An unknown/already-delivered receipt is a no-op, no double publish.
- **CIP-5 — routing by conversation handle.** Same `external_ref` → one thread.
- **CIP-6 — routing event durable.** With the in-proc publish lost (dropping sink = crash-after-commit),
  `MessageReceived` is still durably staged in the outbox — the relay can deliver it. Proven-by-revert.

## Seam (`tests/communication_support_seam.rs`)
- **CSEAM-1 — inbound opens a REAL support ticket.** An inbound WhatsApp message → `MessageReceived` →
  REAL backbone-support `raise_issue`; the ticket carries the party + body. Zero normal Cargo edge.

## §5 round-trip (`scripts/communication_support_seam_roundtrip.sh`)
Regen (`--force`) leaves the seam files (`communication_ports.rs`, `communication_events.rs`,
`communication_write_service.rs`) byte-identical; the oracle + seam re-run green.
