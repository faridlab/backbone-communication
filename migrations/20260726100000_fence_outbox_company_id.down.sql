-- Reverse the ADR-0011 outbox parity migration.
DROP POLICY IF EXISTS outbox_events_company_isolation ON communication.outbox_events;
ALTER TABLE communication.outbox_events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE communication.outbox_events DISABLE ROW LEVEL SECURITY;
DROP INDEX IF EXISTS idx_communication_outbox_company_id;
ALTER TABLE communication.outbox_events DROP COLUMN IF EXISTS company_id;
