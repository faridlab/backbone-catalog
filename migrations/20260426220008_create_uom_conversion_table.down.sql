-- Down: drop catalog.uom_conversions table
DROP TABLE IF EXISTS catalog.uom_conversions CASCADE;
DROP FUNCTION IF EXISTS catalog.uom_conversions_audit_timestamp() CASCADE;
