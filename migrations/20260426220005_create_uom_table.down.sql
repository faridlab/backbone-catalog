-- Down: drop catalog.uoms table
DROP TABLE IF EXISTS catalog.uoms CASCADE;
DROP FUNCTION IF EXISTS catalog.uoms_audit_timestamp() CASCADE;
