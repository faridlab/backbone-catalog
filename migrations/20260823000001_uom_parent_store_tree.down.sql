-- Down: drop the UoM parent-store tree columns and helpers (ADR-0023).
-- Restores the pre-tree flat unit shape.

DROP FUNCTION IF EXISTS catalog.uom_recompute_factors();
DROP INDEX IF EXISTS catalog.idx_uoms_company_relative;
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_factor_positive_chk;
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_shape_chk;
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_uom_fkey;
DROP INDEX IF EXISTS catalog.uoms_id_company_uidx;
ALTER TABLE catalog.uoms DROP COLUMN IF EXISTS factor;
ALTER TABLE catalog.uoms DROP COLUMN IF EXISTS relative_factor;
ALTER TABLE catalog.uoms DROP COLUMN IF EXISTS relative_uom_id;
