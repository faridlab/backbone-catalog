-- Down migration: protected units (UM-4)
-- Reverses the additive flag and the storage-level delete guard. Order matters: the
-- trigger must go before its function, and both before the column.

DROP TRIGGER IF EXISTS uoms_protected_delete_guard ON catalog.uoms;
DROP TRIGGER IF EXISTS uoms_protected_soft_delete_guard ON catalog.uoms;
DROP FUNCTION IF EXISTS catalog.uom_guard_protected_delete();
DROP FUNCTION IF EXISTS catalog.uom_guard_protected_soft_delete();
ALTER TABLE catalog.uoms DROP COLUMN IF EXISTS is_protected;
