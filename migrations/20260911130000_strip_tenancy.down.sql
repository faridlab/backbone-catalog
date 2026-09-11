-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with its plain index and the company isolation policy shape, but restores NO data —
-- rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; treat this
-- down as a schema-shape sketch for archaeology, not a usable rollback.
--
-- The composite UoM-tree FK (relative_uom_id, company_id → uoms (id, company_id)) is
-- restored alongside its supporting unique index so the tree's pre-strip shape is
-- reconstructed; rows whose relative_uom_id crosses what would have been a company
-- boundary satisfy it trivially while company_id is NULL-able, but a real rollback would
-- need the column backfilled and NOT NULL re-armed by hand.

ALTER TABLE catalog.items            ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.item_variants    ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.item_groups      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.brands           ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.attributes       ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.attribute_values ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.uoms             ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE catalog.uom_conversions  ADD COLUMN IF NOT EXISTS company_id uuid;

CREATE INDEX IF NOT EXISTS idx_items_company_id            ON catalog.items (company_id);
CREATE INDEX IF NOT EXISTS idx_item_variants_company_id    ON catalog.item_variants (company_id);
CREATE INDEX IF NOT EXISTS idx_item_groups_company_id      ON catalog.item_groups (company_id);
CREATE INDEX IF NOT EXISTS idx_brands_company_id           ON catalog.brands (company_id);
CREATE INDEX IF NOT EXISTS idx_attributes_company_id       ON catalog.attributes (company_id);
CREATE INDEX IF NOT EXISTS idx_attribute_values_company_id ON catalog.attribute_values (company_id);
CREATE INDEX IF NOT EXISTS idx_uoms_company_id             ON catalog.uoms (company_id);
CREATE INDEX IF NOT EXISTS idx_uom_conversions_company_id  ON catalog.uom_conversions (company_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_items_company_id_item_code
    ON catalog.items (company_id, item_code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_company_id_barcode
    ON catalog.items (company_id, barcode)
    WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_company_id_sku
    ON catalog.item_variants (company_id, sku)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_variants_company_id_barcode
    ON catalog.item_variants (company_id, barcode)
    WHERE barcode IS NOT NULL AND (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_groups_company_id_code
    ON catalog.item_groups (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_brands_company_id_code
    ON catalog.brands (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attributes_company_id_code
    ON catalog.attributes (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attribute_values_company_id_attribute_id_code
    ON catalog.attribute_values (company_id, attribute_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uoms_company_id_code
    ON catalog.uoms (company_id, code)
    WHERE (metadata ->> 'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_uom_conversions_company_id_from_uom_id_to_uom_id
    ON catalog.uom_conversions (company_id, from_uom_id, to_uom_id)
    WHERE (metadata ->> 'deleted_at') IS NULL;

CREATE POLICY items_company_isolation ON catalog.items
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY item_variants_company_isolation ON catalog.item_variants
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY item_groups_company_isolation ON catalog.item_groups
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY brands_company_isolation ON catalog.brands
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY attributes_company_isolation ON catalog.attributes
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY attribute_values_company_isolation ON catalog.attribute_values
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY uoms_company_isolation ON catalog.uoms
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY uom_conversions_company_isolation ON catalog.uom_conversions
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Restore the parent-store tree's company-keyed FK shape: the supporting (id, company_id)
-- unique index, then the composite FK replacing the plain one the strip installed.
CREATE UNIQUE INDEX IF NOT EXISTS uoms_id_company_uidx ON catalog.uoms (id, company_id);
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_uom_fkey;
DO $$
BEGIN
    IF to_regclass('catalog.uoms') IS NOT NULL
       AND EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = 'catalog' AND table_name = 'uoms' AND column_name = 'relative_uom_id'
       )
       AND NOT EXISTS (
           SELECT 1 FROM pg_constraint
           WHERE conname = 'uoms_relative_uom_fkey'
             AND conrelid = 'catalog.uoms'::regclass
       ) THEN
        ALTER TABLE catalog.uoms
            ADD CONSTRAINT uoms_relative_uom_fkey
            FOREIGN KEY (relative_uom_id, company_id) REFERENCES catalog.uoms (id, company_id);
    END IF;
END $$;
CREATE INDEX IF NOT EXISTS idx_uoms_company_relative
    ON catalog.uoms (company_id, relative_uom_id)
    WHERE relative_uom_id IS NOT NULL;
