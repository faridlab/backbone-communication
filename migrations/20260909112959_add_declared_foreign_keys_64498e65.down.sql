-- Down: drop the declared foreign keys for communication module
ALTER TABLE IF EXISTS communication.messages DROP CONSTRAINT IF EXISTS fk_messages_thread_id;
