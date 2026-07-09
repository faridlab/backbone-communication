-- Down: drop communication.threads table
DROP TABLE IF EXISTS communication.threads CASCADE;
DROP FUNCTION IF EXISTS communication.threads_audit_timestamp() CASCADE;
