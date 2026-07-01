-- Down: drop catalog.item_variants table
DROP TABLE IF EXISTS catalog.item_variants CASCADE;
DROP FUNCTION IF EXISTS catalog.item_variants_audit_timestamp() CASCADE;
