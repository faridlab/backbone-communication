-- The transactional outbox for this module's own schema (mirrors backbone_outbox::outbox::migrate). The
-- routing event MessageReceived is staged here INSIDE the same tx as the message insert, so the event and
-- the message commit atomically — a crash between commit and publish can never drop it (maturity council
-- 2026-07-08). A relay drains outbox_events at-least-once; consumers dedup via inbox_consumed.
CREATE TABLE IF NOT EXISTS communication.outbox_events (
  id             uuid PRIMARY KEY,
  event_type     text NOT NULL,
  aggregate_type text NOT NULL,
  aggregate_id   text NOT NULL,
  payload        jsonb NOT NULL,
  occurred_at    timestamptz NOT NULL,
  correlation_id text,
  causation_id   text,
  version        int NOT NULL DEFAULT 1,
  created_at     timestamptz NOT NULL DEFAULT now(),
  published_at   timestamptz
);
CREATE INDEX IF NOT EXISTS idx_communication_outbox_unpublished
  ON communication.outbox_events (occurred_at) WHERE published_at IS NULL;

CREATE TABLE IF NOT EXISTS communication.inbox_consumed (
  consumer    text NOT NULL,
  event_id    uuid NOT NULL,
  consumed_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (consumer, event_id)
);
