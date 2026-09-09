-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with its plain index and the company isolation policy shape, but restores NO data —
-- rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; treat this
-- down as a schema-shape sketch for archaeology, not a usable rollback.

ALTER TABLE communication.messages ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE communication.threads  ADD COLUMN IF NOT EXISTS company_id uuid;

-- The strip's restored tenant-free party lookup index goes away again (the company-leading
-- variant would need company data this sketch does not restore).
DROP INDEX IF EXISTS communication.idx_threads_party_id;

CREATE INDEX IF NOT EXISTS idx_threads_company_id_party_id ON communication.threads (company_id, party_id);

CREATE POLICY threads_company_isolation ON communication.threads
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY messages_company_isolation ON communication.messages
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
