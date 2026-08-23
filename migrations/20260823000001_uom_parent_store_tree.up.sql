-- Migration: UoM parent-store tree (ADR-0023)
-- Hand-authored (user-owned). Not regenerated.
--
-- Ports the v19 unit-of-measure shape: units form reference trees via relative_uom_id
-- (NULL = tree root); relative_factor is the per-link ratio (1 of this unit = relative_factor
-- of its parent unit); `factor` is the recursive STORED effective factor to the tree's root,
-- derived on every tree write — never hand-written by callers. The pre-v19 category shape
-- (uom.category, factor_inv) was removed upstream in v19 and stays deleted (ADR-0023).
--
-- Everything here is idempotent: IF NOT EXISTS guards on every DDL, and the backfill only
-- writes rows whose stored factor actually differs from the derived one, so re-running the
-- backfill (or the write-path recompute function) is a no-op on a consistent tree.

-- 1. Tree columns.
ALTER TABLE catalog.uoms ADD COLUMN IF NOT EXISTS relative_uom_id UUID;
ALTER TABLE catalog.uoms ADD COLUMN IF NOT EXISTS relative_factor NUMERIC(24,12);
ALTER TABLE catalog.uoms ADD COLUMN IF NOT EXISTS factor NUMERIC(24,12) NOT NULL DEFAULT 1;

-- 2. Same-company parent. A unit's reference unit must live in the owning company's tree;
--    the composite FK makes a cross-company link unrepresentable at the storage layer. The
--    extra unique index exists only to give the FK its referencing shape (id is already the
--    primary key, so (id, company_id) is unique by construction).
CREATE UNIQUE INDEX IF NOT EXISTS uoms_id_company_uidx ON catalog.uoms (id, company_id);
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_uom_fkey;
ALTER TABLE catalog.uoms
    ADD CONSTRAINT uoms_relative_uom_fkey
    FOREIGN KEY (relative_uom_id, company_id)
    REFERENCES catalog.uoms (id, company_id);

-- 3. Shape and positivity invariants. relative_factor is required exactly when a parent is
--    set, and every ratio/factor must be positive.
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_relative_shape_chk;
ALTER TABLE catalog.uoms
    ADD CONSTRAINT uoms_relative_shape_chk CHECK (
        (relative_uom_id IS NULL AND relative_factor IS NULL)
        OR (relative_uom_id IS NOT NULL AND relative_factor IS NOT NULL AND relative_factor > 0)
    );
ALTER TABLE catalog.uoms DROP CONSTRAINT IF EXISTS uoms_factor_positive_chk;
ALTER TABLE catalog.uoms
    ADD CONSTRAINT uoms_factor_positive_chk CHECK (factor > 0);

CREATE INDEX IF NOT EXISTS idx_uoms_company_relative
    ON catalog.uoms (company_id, relative_uom_id)
    WHERE relative_uom_id IS NOT NULL;

-- 4. Write-path recompute: re-derive every stored factor from the roots down all chains.
--    Runs under the invoker's RLS scope, so an application call recomputes exactly the
--    caller's company trees; migrations/owner calls see the whole table.
--
--    The depth cap bounds recursion when stored links are corrupt (a chain that walks into
--    a cycle would otherwise recurse forever); rows past the cap — cycle members and any
--    row otherwise unreachable from a root — make the function FAIL LOUDLY instead of
--    silently pinning a stale factor.
CREATE OR REPLACE FUNCTION catalog.uom_recompute_factors() RETURNS void AS $$
DECLARE
    unreachable integer;
BEGIN
    WITH RECURSIVE tree AS (
        SELECT id, 1::numeric(24,12) AS root_factor, 0 AS depth
        FROM catalog.uoms
        WHERE relative_uom_id IS NULL
        UNION ALL
        SELECT c.id, (t.root_factor * c.relative_factor)::numeric(24,12), t.depth + 1
        FROM catalog.uoms c
        JOIN tree t ON c.relative_uom_id = t.id
        WHERE t.depth < 64
    ),
    updated AS (
        UPDATE catalog.uoms u
        SET factor = t.root_factor
        FROM tree t
        WHERE u.id = t.id
          AND u.factor IS DISTINCT FROM t.root_factor
        RETURNING 1
    )
    SELECT count(*) INTO unreachable
    FROM catalog.uoms u
    WHERE NOT EXISTS (SELECT 1 FROM tree t WHERE t.id = u.id);

    IF unreachable > 0 THEN
        RAISE EXCEPTION
            'uom factor tree: % unit(s) unreachable from a tree root (cycle or dangling relative_uom_id) — stored factors are not derivable',
            unreachable;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- 5. Backfill the stored factor over existing rows. Existing flat rows have no tree links, so
--    they are all roots and land on factor = 1; rows that already carry links (pre-seeded
--    chains) resolve recursively in one pass. Correct for chains, idempotent on re-run.
SELECT catalog.uom_recompute_factors();
