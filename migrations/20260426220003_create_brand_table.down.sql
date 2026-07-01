-- Down: drop catalog.brands table
DROP TABLE IF EXISTS catalog.brands CASCADE;
DROP FUNCTION IF EXISTS catalog.brands_audit_timestamp() CASCADE;
