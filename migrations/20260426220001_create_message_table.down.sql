-- Down: drop communication.messages table
DROP TABLE IF EXISTS communication.messages CASCADE;
DROP FUNCTION IF EXISTS communication.messages_audit_timestamp() CASCADE;
