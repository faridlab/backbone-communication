# backbone-communication — Extension Guide

## Public surface (stable)
- **Events** (`application::service::communication_events`): `MessageReceived` (the inbound-routing
  signal), `MessageDelivered`, `MessageFailed`, the `CommunicationEvent` union, and
  `CommunicationEventSink`. CRM/support subscribe to `MessageReceived` (over the bus, or by draining the
  transactional outbox) to capture a lead / open a ticket. The event carries everything a consumer needs
  (company, party, channel, subject ref, from, body) — it never re-queries this module.
- **Channel port** (`application::service::communication_ports`): `ChannelPort` + DTOs (`OutboundSend`,
  `ChannelAck`, `ChannelRejected`) — the outbound seam a composing service implements over a real WhatsApp/
  email/SMS provider. Zero transport dependency in the shipped library.
- **Write path** (`application::service::communication_write_service::CommunicationWriteService`): the
  guarded verbs (`receive_inbound`, `open_thread`, `link_thread`, `send_outbound`, `mark_delivered`,
  `close_thread`).
- **Durability**: `MessageReceived` is staged in this module's `communication.outbox_events` in the same tx
  as the message. A composing service runs a relay (`backbone_outbox::relay`) to deliver it to the bus and
  a consumer's inbox makes the effect exactly-once.

## How a consuming service uses communication
Implement `ChannelPort` over your provider to send. To route inbound traffic, either subscribe to
`MessageReceived` on the bus or drain the outbox; open your ticket/lead from the event fields alone, then
optionally call `link_thread(subject_type, subject_id)` to record what the thread now concerns. Never
mutate communication's tables directly.

## Not a contract
- The 12 generated CRUD endpoints per entity are convenience scaffolding. Do **not** insert a message or
  flip a status through the generic PATCH surface — it bypasses the inbound dedup, the outbox staging, and
  the send/receipt gating. Use `CommunicationWriteService`.
- `// <<< CUSTOM` blocks preserve local edits only; not a cross-module extension point.

## Invariants a consumer must not break
- One message per `(channel, external_id)`; `receive_inbound` is the only inbound writer.
- `MessageReceived` is emitted exactly once per new inbound message and is durable (committed with the row).
- A `sent → delivered` transition happens at most once; a closed thread sends nothing.
