# backbone-communication — PRD

Channels pillar (Tier 5) · the **channel gateway** · posts **no GL** · publishes inbound events.

## Why
An Indonesia SMB lives on **WhatsApp**. Several built modules parked the same need — CRM's "inbound
WhatsApp → lead", support's "email/WhatsApp → ticket", every module's "notify the customer". That is one
shared bounded context, not per-module glue. `backbone-communication` is the gateway: it records inbound
and outbound messages on threads and turns an inbound message into a **domain event** that CRM, support,
etc. subscribe to **over the event bus — not a driven call**.

## Scope (KEEP — tier5-deferred.md §4)
- **Thread** — a conversation with a party over one channel (WhatsApp/email/SMS), optionally linked to
  the business object it concerns (a lead, an issue, an order) via a polymorphic logical reference
  (`subject_type` + `subject_id`).
- **Message** — one inbound or outbound message on a thread. **Inbound is deduped on (channel,
  external_id)** — provider webhooks are at-least-once, so a redelivery must not create a second message
  nor re-route.
- **Inbound routing** — `receive_inbound` records the message, routes it onto a thread (by provider
  conversation handle, else party, else a new thread), and publishes `MessageReceived` **exactly once**
  so a consumer (CRM captures a lead, support opens/appends an issue) can act. Retires CRM's "inbound
  WhatsApp adapter" and support's "email-to-ticket ingestion" parks.
- **Outbound send** — `send_outbound` records a queued message and sends it through the `ChannelPort`
  (the WhatsApp Business API / email / SMS adapter), tracking status (queued → sent → delivered/failed).
- **Delivery receipts** — `mark_delivered` transitions a sent message once (idempotent).

## Non-goals (CUT / DEFER — tier5-deferred.md §4)
- Full omnichannel **campaign automation** (that's marketing — explicitly cut for SMB), IVR, social
  listening.
- The concrete WhatsApp/email/SMS provider adapters — those are wired behind `ChannelPort` by a composing
  service (the module ships the port, not an SDK).
- Telephony / call logging (→ `backbone-telephony`), EDI (→ `backbone-edi`).
- Templated fan-out of outbound notifications (→ `backbone-notification`, which dispatches *through* this
  module's channels).

## Success criteria
- An inbound message routes **exactly once** even under at-least-once webhook redelivery, and the routing
  event is **durable** (survives a crash between the DB commit and the publish).
- An inbound message opens a real support ticket (proven against REAL backbone-support).
- Zero normal Cargo edge; survives a full codegen regen (§5). Posts no GL.
