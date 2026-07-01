-- Down: drop catalog.attributes table
DROP TABLE IF EXISTS catalog.attributes CASCADE;
DROP FUNCTION IF EXISTS catalog.attributes_audit_timestamp() CASCADE;
