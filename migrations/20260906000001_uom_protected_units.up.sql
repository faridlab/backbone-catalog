-- Migration: protected units (UM-4)
-- Hand-authored (user-owned). Not regenerated.
--
-- Ports upstream's protected-unit semantics: units seeded as canonical reference data
-- by the platform cannot be deleted, while user-created units stay deletable. Upstream
-- keys protection off ir.model.data XML-IDs and enforces it with an ORM-level
-- @api.ondelete hook — a guard that raw SQL silently bypasses. This port moves the guard
-- into the database itself (the same posture as the conversion guards: the primitive
-- fails loudly, so no caller — validated service, generic CRUD, or raw SQL — can bypass it).
--
-- Existing-rows decision: purely additive. The column lands NOT NULL DEFAULT false, so
-- every pre-existing row stays deletable exactly as before (nothing is retroactively
-- reclassified as protected). No module-seeded units exist to backfill — the seeding
-- mechanism that will mark canonical reference units (is_protected = true at insert) has
-- not landed yet; when it does, protection is a property of the seed write, not of a
-- migration guessing at row provenance. Idempotent: IF NOT EXISTS / CREATE OR REPLACE
-- throughout, and the trigger is dropped before recreate.

-- 1. The protection flag. false = user-created (deletable); true = module-seeded
--    reference data (delete-refused). Set by the seeding path, never by API callers.
ALTER TABLE catalog.uoms ADD COLUMN IF NOT EXISTS is_protected BOOLEAN NOT NULL DEFAULT false;

-- 2. Storage-level on-delete guard (the @api.ondelete analog). A BEFORE DELETE trigger
--    raises on any attempt to hard-delete a protected row. Soft delete (archive) is an
--    UPDATE of metadata->>'deleted_at' and stays available to the validated retire path,
--    which applies its own protection check before archiving.
CREATE OR REPLACE FUNCTION catalog.uom_guard_protected_delete() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION
        'unit % (%) is module-seeded reference data and cannot be deleted',
        OLD.code, OLD.id
        USING ERRCODE = '23506',  -- foreign_key_violation shape: referenced row
        CONSTRAINT = 'uoms_protected_units_guard',
        TABLE = 'uoms';
    RETURN OLD;  -- unreachable; the RAISE aborts the delete
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS uoms_protected_delete_guard ON catalog.uoms;
CREATE TRIGGER uoms_protected_delete_guard
    BEFORE DELETE ON catalog.uoms
    FOR EACH ROW
    WHEN (OLD.is_protected)
    EXECUTE FUNCTION catalog.uom_guard_protected_delete();

-- 3. Soft-delete guard. Deletion in this codebase is normally a soft delete (stamping
--    metadata->>'deleted_at'), which a row-level DELETE trigger never sees. This guard
--    refuses the not-deleted -> deleted transition on protected rows so the generic CRUD
--    soft-delete path (and any hand-written UPDATE) hits the same wall as a hard delete.
--    Retiring a protected unit stays possible through the status lifecycle
--    (active -> inactive) — that is archiving, not deletion. Restoring (clearing
--    deleted_at) is untouched: the guard only fires on the delete-direction transition.
CREATE OR REPLACE FUNCTION catalog.uom_guard_protected_soft_delete() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION
        'unit % (%) is module-seeded reference data and cannot be deleted (soft delete refused; retire via status instead)',
        NEW.code, NEW.id
        USING ERRCODE = '23506',
        CONSTRAINT = 'uoms_protected_units_guard',
        TABLE = 'uoms';
    RETURN NEW;  -- unreachable; the RAISE aborts the update
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS uoms_protected_soft_delete_guard ON catalog.uoms;
CREATE TRIGGER uoms_protected_soft_delete_guard
    BEFORE UPDATE ON catalog.uoms
    FOR EACH ROW
    WHEN (OLD.is_protected
          AND (OLD.metadata->>'deleted_at') IS NULL
          AND (NEW.metadata->>'deleted_at') IS NOT NULL)
    EXECUTE FUNCTION catalog.uom_guard_protected_soft_delete();
