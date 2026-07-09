-- Down: drop enum types for communication module
DROP TYPE IF EXISTS thread_status CASCADE;
DROP TYPE IF EXISTS channel CASCADE;
DROP TYPE IF EXISTS message_status CASCADE;
DROP TYPE IF EXISTS direction CASCADE;
