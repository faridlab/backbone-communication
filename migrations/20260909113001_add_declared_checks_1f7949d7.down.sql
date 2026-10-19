-- Down: drop the declared rule constraints for communication module
ALTER TABLE IF EXISTS communication.threads DROP CONSTRAINT IF EXISTS ck_threads_external_ref_max_length__max_length;
ALTER TABLE IF EXISTS communication.threads DROP CONSTRAINT IF EXISTS ck_threads_subject_type_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_failure_reason_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_body_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_body_required__required;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_address_to_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_address_from_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_external_id_max_length__max_length;
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS ck_messages_thread_id_required__required;
DO $$
BEGIN
    IF to_regclass('communication.uq_messages_external_id__unique') IS NOT NULL AND to_regclass('communication.idx_messages_channel_external_id') IS NULL THEN
        ALTER INDEX communication.uq_messages_external_id__unique RENAME TO idx_messages_channel_external_id;
    END IF;
END $$;

