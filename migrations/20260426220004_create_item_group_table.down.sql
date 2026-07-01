-- Down: drop catalog.item_groups table
DROP TABLE IF EXISTS catalog.item_groups CASCADE;
DROP FUNCTION IF EXISTS catalog.item_groups_audit_timestamp() CASCADE;
