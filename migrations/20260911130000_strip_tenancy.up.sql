-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the catalog tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading indexes and uniques,
-- the <table>_company_isolation RLS policy, and the company_id column itself.
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those now.
--
-- UoM tree note: the parent-store tree (ADR-0023) keyed its composite FK on company_id
-- (relative_uom_id, company_id → uoms (id, company_id)). The strip re-keys that FK to the
-- plain single-column shape (relative_uom_id → uoms (id)) so the tree survives without the
-- tenancy column; the down restores the composite shape.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'items', 'item_variants', 'item_groups', 'brands',
        'attributes', 'attribute_values', 'uoms', 'uom_conversions'
    ]
    LOOP
        IF to_regclass(format('catalog.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'catalog' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM catalog.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM catalog.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' catalog.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── items ─────────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_items_company_id_item_code;
DROP INDEX IF EXISTS catalog.idx_items_company_id_barcode;
DROP INDEX IF EXISTS catalog.idx_items_company_id;
DROP POLICY IF EXISTS items_company_isolation ON catalog.items;
ALTER TABLE catalog.items DROP COLUMN IF EXISTS company_id;

-- ── item_variants ─────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_item_variants_company_id_sku;
DROP INDEX IF EXISTS catalog.idx_item_variants_company_id_barcode;
DROP INDEX IF EXISTS catalog.idx_item_variants_company_id;
DROP POLICY IF EXISTS item_variants_company_isolation ON catalog.item_variants;
ALTER TABLE catalog.item_variants DROP COLUMN IF EXISTS company_id;

-- ── item_groups ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_item_groups_company_id_code;
DROP INDEX IF EXISTS catalog.idx_item_groups_company_id;
DROP POLICY IF EXISTS item_groups_company_isolation ON catalog.item_groups;
ALTER TABLE catalog.item_groups DROP COLUMN IF EXISTS company_id;

-- ── brands ────────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_brands_company_id_code;
DROP INDEX IF EXISTS catalog.idx_brands_company_id;
DROP POLICY IF EXISTS brands_company_isolation ON catalog.brands;
ALTER TABLE catalog.brands DROP COLUMN IF EXISTS company_id;

-- ── attributes ────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_attributes_company_id_code;
DROP INDEX IF EXISTS catalog.idx_attributes_company_id;
DROP POLICY IF EXISTS attributes_company_isolation ON catalog.attributes;
ALTER TABLE catalog.attributes DROP COLUMN IF EXISTS company_id;

-- ── attribute_values ──────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_attribute_values_company_id_attribute_id_code;
DROP INDEX IF EXISTS catalog.idx_attribute_values_company_id;
DROP POLICY IF EXISTS attribute_values_company_isolation ON catalog.attribute_values;
ALTER TABLE catalog.attribute_values DROP COLUMN IF EXISTS company_id;

-- ── uoms (plus the parent-store tree's company-keyed FK artifacts) ───────────
-- Order matters: the composite FK (relative_uom_id, company_id) REFERENCES (id, company_id)
-- depends on the uoms_id_company_uidx unique index, so the FK goes first.
DROP INDEX IF EXISTS catalog.idx_uoms_company_relative;
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_uom_fkey;
DROP INDEX IF EXISTS catalog.uoms_id_company_uidx;
DO $$
BEGIN
    IF to_regclass('catalog.uoms') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM pg_constraint
           WHERE conname = 'uoms_relative_uom_fkey'
             AND conrelid = 'catalog.uoms'::regclass
       ) THEN
        ALTER TABLE catalog.uoms
            ADD CONSTRAINT uoms_relative_uom_fkey
            FOREIGN KEY (relative_uom_id) REFERENCES catalog.uoms (id);
    END IF;
END $$;
DROP INDEX IF EXISTS catalog.idx_uoms_company_id_code;
DROP INDEX IF EXISTS catalog.idx_uoms_company_id;
DROP POLICY IF EXISTS uoms_company_isolation ON catalog.uoms;
ALTER TABLE catalog.uoms DROP COLUMN IF EXISTS company_id;

-- ── uom_conversions ───────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_uom_conversions_company_id_from_uom_id_to_uom_id;
DROP INDEX IF EXISTS catalog.idx_uom_conversions_company_id;
DROP POLICY IF EXISTS uom_conversions_company_isolation ON catalog.uom_conversions;
ALTER TABLE catalog.uom_conversions DROP COLUMN IF EXISTS company_id;
