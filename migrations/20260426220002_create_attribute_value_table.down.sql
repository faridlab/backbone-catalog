-- Down: drop catalog.attribute_values table
DROP TABLE IF EXISTS catalog.attribute_values CASCADE;
DROP FUNCTION IF EXISTS catalog.attribute_values_audit_timestamp() CASCADE;
