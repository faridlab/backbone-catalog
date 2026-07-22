-- Down migration: revert tenant-scope catalog (ADR-0010 Decision B1).
-- Best-effort. Restores global unique indexes; drops the RLS fence and company_id.
-- WARNING: rows belonging to >1 company cannot be re-collapsed to a single global
-- unique index without conflicts — only run this down if you are sure every
-- company shares globally-unique codes, or after deduplicating.

-- Drop RLS fence + policy on every table (no-op if never armed).
DO $$
DECLARE
    t text;
    tabs text[] := ARRAY[
        'items','item_variants','item_groups','brands',
        'attributes','attribute_values','uoms','uom_conversions'
    ];
BEGIN
    FOREACH t IN ARRAY tabs LOOP
        EXECUTE format('DROP POLICY IF EXISTS %I ON catalog.%I', t || '_company_isolation', t);
        EXECUTE format('ALTER TABLE catalog.%I NO FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE catalog.%I DISABLE ROW LEVEL SECURITY', t);
    END LOOP;
END $$;

-- Drop composite per-company unique indexes, restore global uniques.
DROP INDEX IF EXISTS catalog.idx_items_company_id_item_code;
DROP INDEX IF EXISTS catalog.idx_items_company_id_barcode;
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_item_code
    ON catalog.items (item_code) WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_barcode
    ON catalog.items (barcode) WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_item_variants_company_id_sku;
DROP INDEX IF EXISTS catalog.idx_item_variants_company_id_barcode;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_sku
    ON catalog.item_variants (sku) WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_barcode
    ON catalog.item_variants (barcode) WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_item_groups_company_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_groups_code
    ON catalog.item_groups (code) WHERE (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_brands_company_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_brands_code
    ON catalog.brands (code) WHERE (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_attributes_company_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attributes_code
    ON catalog.attributes (code) WHERE (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_attribute_values_company_id_attribute_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attribute_values_attribute_id_code
    ON catalog.attribute_values (attribute_id, code) WHERE (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_uoms_company_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uoms_code
    ON catalog.uoms (code) WHERE (metadata ->> 'deleted_at') IS NULL;

DROP INDEX IF EXISTS catalog.idx_uom_conversions_company_id_from_uom_id_to_uom_id;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uom_conversions_from_uom_id_to_uom_id
    ON catalog.uom_conversions (from_uom_id, to_uom_id) WHERE (metadata ->> 'deleted_at') IS NULL;

-- Drop supporting indexes + the column.
DROP INDEX IF EXISTS catalog.idx_items_company_id;
DROP INDEX IF EXISTS catalog.idx_item_variants_company_id;
DROP INDEX IF EXISTS catalog.idx_item_groups_company_id;
DROP INDEX IF EXISTS catalog.idx_brands_company_id;
DROP INDEX IF EXISTS catalog.idx_attributes_company_id;
DROP INDEX IF EXISTS catalog.idx_attribute_values_company_id;
DROP INDEX IF EXISTS catalog.idx_uoms_company_id;
DROP INDEX IF EXISTS catalog.idx_uom_conversions_company_id;

ALTER TABLE catalog.items            DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.item_variants    DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.item_groups      DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.brands           DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.attributes       DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.attribute_values DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.uoms             DROP COLUMN IF EXISTS company_id;
ALTER TABLE catalog.uom_conversions  DROP COLUMN IF EXISTS company_id;
