-- Down: drop catalog.items table
DROP TABLE IF EXISTS catalog.items CASCADE;
DROP FUNCTION IF EXISTS catalog.items_audit_timestamp() CASCADE;
