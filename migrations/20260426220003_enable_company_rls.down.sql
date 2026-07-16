-- Down: remove the company RLS fence for communication module

-- Reverse the company RLS fence for communication.messages
DROP POLICY IF EXISTS messages_company_isolation ON communication.messages;
ALTER TABLE communication.messages NO FORCE ROW LEVEL SECURITY;
ALTER TABLE communication.messages DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for communication.threads
DROP POLICY IF EXISTS threads_company_isolation ON communication.threads;
ALTER TABLE communication.threads NO FORCE ROW LEVEL SECURITY;
ALTER TABLE communication.threads DISABLE ROW LEVEL SECURITY;

