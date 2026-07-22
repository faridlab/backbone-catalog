-- Migration: tenant-scope catalog (ADR-0010 Decision B1)
-- Hand-authored (user-owned). Not regenerated.
--
-- Catalog was the last fully global module in the ERP — zero company_id, zero RLS.
-- This migration makes every catalog entity tenant-scoped and fences each table
-- with the ADR-0008 RLS invariant:
--
--     company_id = NULLIF(current_setting('app.company_id', true), '')::uuid
--
-- Per-table shape: ADD COLUMN company_id UUID → backfill → SET NOT NULL →
-- ENABLE + FORCE RLS → POLICY. Global unique indexes (item_code, barcode, sku,
-- uom_conversion pair, attribute+code, brand code, item_group code, uom code) are
-- replaced with composite (company_id, ...) per-company uniques, preserving the
-- `deleted_at IS NULL` soft-delete WHERE clause.
--
-- BACKFILL + FENCE POLICY (ADR-0010 B1, resolved 2026-07-22):
--   - If `organization.companies` has exactly one live row, backfill every catalog
--     row to it (convenience for the single-company / dev / demo case).
--   - The RLS fence (NOT NULL + ENABLE + FORCE + POLICY) is then armed UNCONDITIONALLY
--     on every table that has zero NULL company_id rows.
--   - If ANY catalog row still has NULL company_id after backfill (the multi-company
--     or no-organization case with existing data), the migration FAILS LOUD —
--     RAISE EXCEPTION naming every stray table + row count — rather than silently
--     leaving the fence disarmed. The operator must assign those rows (or confirm a
--     fresh DB), then re-run; the migration is idempotent and will arm the fence once
--     clean. We never pick an arbitrary company_id, and we never ship a disarmed
--     fence in the multi-tenant case where it is needed most.
--
-- No SQL FK to organization.companies is added: catalog is a framework module and
-- must stay independently deployable. RLS is the fence, not the FK (matches pos).

-- ==============================================================================
-- Step 1: ADD COLUMN company_id UUID (nullable) on every catalog table.
-- ==============================================================================

ALTER TABLE catalog.items            ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.item_variants    ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.item_groups      ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.brands           ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.attributes       ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.attribute_values ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.uoms             ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE catalog.uom_conversions  ADD COLUMN IF NOT EXISTS company_id UUID;

-- Supporting index for the per-company queries (added unconditionally; cheap).
CREATE INDEX IF NOT EXISTS idx_items_company_id            ON catalog.items (company_id);
CREATE INDEX IF NOT EXISTS idx_item_variants_company_id    ON catalog.item_variants (company_id);
CREATE INDEX IF NOT EXISTS idx_item_groups_company_id      ON catalog.item_groups (company_id);
CREATE INDEX IF NOT EXISTS idx_brands_company_id           ON catalog.brands (company_id);
CREATE INDEX IF NOT EXISTS idx_attributes_company_id       ON catalog.attributes (company_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_company_id ON catalog.attribute_values (company_id);
CREATE INDEX IF NOT EXISTS idx_uoms_company_id             ON catalog.uoms (company_id);
CREATE INDEX IF NOT EXISTS idx_uom_conversions_company_id  ON catalog.uom_conversions (company_id);

-- ==============================================================================
-- Step 2: BACKFILL — only when exactly one live company exists (convenience).
-- Multi-company / no-org deployments skip backfill; Step 4 then fails loud on any
-- remaining NULL rows so the fence is never silently disarmed.
-- ==============================================================================

DO $$
DECLARE
    has_org boolean;
    cnt     int;
    cid     uuid;
    t       text;
    tabs    text[] := ARRAY[
        'items','item_variants','item_groups','brands',
        'attributes','attribute_values','uoms','uom_conversions'
    ];
BEGIN
    SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = 'organization') INTO has_org;
    IF has_org THEN
        EXECUTE $q$
            SELECT COUNT(*) FROM organization.companies
            WHERE (metadata ->> 'deleted_at') IS NULL
        $q$ INTO cnt;
    ELSE
        -- organization schema not installed in this deployment → unresolvable.
        cnt := -1;
    END IF;

    IF cnt = 1 THEN
        EXECUTE $q$
            SELECT id FROM organization.companies
            WHERE (metadata ->> 'deleted_at') IS NULL
            LIMIT 1
        $q$ INTO cid;

        RAISE NOTICE 'catalog ADR-0010 B1: exactly 1 company (%) — backfilling % tables', cid, array_length(tabs, 1);
        FOREACH t IN ARRAY tabs LOOP
            EXECUTE format('UPDATE catalog.%I SET company_id = $1 WHERE company_id IS NULL', t)
                USING cid;
        END LOOP;
    ELSE
        -- AMBIGUOUS (0 or >1 companies, or no organization schema). Do NOT backfill and
        -- do NOT pick an arbitrary company. Step 4 will fail loud if any NULL rows remain.
        RAISE NOTICE
            'catalog ADR-0010 B1: backfill skipped (organization.companies live-row count=%). '
            'Step 4 will fail loud on any catalog rows still missing company_id.',
            cnt;
    END IF;
END $$;

-- ==============================================================================
-- Step 3: UNIQUE INDEX change — drop global, create per-company composite.
-- (Always applied: the column is nullable-safe with the partial WHERE, and once
--  backfill is resolved later these indexes must already be per-company.)
-- ==============================================================================

-- ── items: item_code, barcode ────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_items_item_code;
DROP INDEX IF EXISTS catalog.idx_items_barcode;
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_company_id_item_code
    ON catalog.items (company_id, item_code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_company_id_barcode
    ON catalog.items (company_id, barcode)
    WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;

-- ── item_variants: sku, barcode ──────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_item_variants_sku;
DROP INDEX IF EXISTS catalog.idx_item_variants_barcode;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_company_id_sku
    ON catalog.item_variants (company_id, sku)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_company_id_barcode
    ON catalog.item_variants (company_id, barcode)
    WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;

-- ── item_groups: code ────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_item_groups_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_groups_company_id_code
    ON catalog.item_groups (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ── brands: code ─────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_brands_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_brands_company_id_code
    ON catalog.brands (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ── attributes: code ─────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_attributes_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attributes_company_id_code
    ON catalog.attributes (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ── attribute_values: (attribute_id, code) ───────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_attribute_values_attribute_id_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attribute_values_company_id_attribute_id_code
    ON catalog.attribute_values (company_id, attribute_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ── uoms: code ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_uoms_code;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uoms_company_id_code
    ON catalog.uoms (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ── uom_conversions: (from_uom_id, to_uom_id) ────────────────────────────────
DROP INDEX IF EXISTS catalog.idx_uom_conversions_from_uom_id_to_uom_id;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uom_conversions_company_id_from_uom_id_to_uom_id
    ON catalog.uom_conversions (company_id, from_uom_id, to_uom_id)
    WHERE (metadata ->> 'deleted_at') IS NULL;

-- ==============================================================================
-- Step 4: FAIL LOUD on strays, then arm the fence on ALL tables atomically.
-- First sweep counts NULL-company_id rows per table. If ANY exist, RAISE EXCEPTION
-- listing every stray table + count and abort — the fence is never partially armed
-- and never silently disarmed in the multi-tenant case. If all clean, arm all 8.
-- Idempotent: after the operator assigns strays, re-running arms the fence.
-- ==============================================================================

DO $$
DECLARE
    t            text;
    null_rows    int;
    tabs         text[] := ARRAY[
        'items','item_variants','item_groups','brands',
        'attributes','attribute_values','uoms','uom_conversions'
    ];
    strays       text[] := ARRAY[]::text[];
BEGIN
    -- Sweep: collect every table that still has unassigned rows.
    FOREACH t IN ARRAY tabs LOOP
        EXECUTE format('SELECT COUNT(*) FROM catalog.%I WHERE company_id IS NULL', t) INTO null_rows;
        IF null_rows > 0 THEN
            strays := array_append(strays, format('%I=%s', t, null_rows));
        END IF;
    END LOOP;

    -- Fail loud if anything is unresolved — do NOT ship a disarmed fence.
    IF array_length(strays, 1) IS NOT NULL THEN
        RAISE EXCEPTION
            'catalog ADR-0010 B1: refusing to fence — % catalog table(s) still have NULL company_id (%). '
            'Assign every catalog row to a tenant (or confirm a fresh DB), then re-run this migration. '
            'No RLS fence has been armed.',
            array_length(strays, 1), array_to_string(strays, ', ');
    END IF;

    -- All clean → arm the fence on every table.
    FOREACH t IN ARRAY tabs LOOP
        EXECUTE format('ALTER TABLE catalog.%I ALTER COLUMN company_id SET NOT NULL', t);
        EXECUTE format('ALTER TABLE catalog.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE catalog.%I FORCE  ROW LEVEL SECURITY', t);
        EXECUTE format(
            'DROP POLICY IF EXISTS %I ON catalog.%I; '
            'CREATE POLICY %I ON catalog.%I FOR ALL '
            'USING      (company_id = NULLIF(current_setting(''app.company_id'', true), '''')::uuid) '
            'WITH CHECK (company_id = NULLIF(current_setting(''app.company_id'', true), '''')::uuid)',
            t || '_company_isolation', t,
            t || '_company_isolation', t
        );
    END LOOP;

    RAISE NOTICE 'catalog ADR-0010 B1: RLS fence live on all 8 catalog tables.';
END $$;
