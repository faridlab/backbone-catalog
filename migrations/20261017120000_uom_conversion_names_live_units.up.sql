-- A conversion names live units only. The foreign keys refuse a missing unit but not a
-- soft-deleted one, so this trigger reads both units with FOR SHARE: a conversion written while
-- a unit is being deleted waits for the delete and is then refused, and the delete, which locks
-- the unit first, finds the conversion and refuses. The refusal carries the rule's code
-- (`uom_not_found`) on the column that names the deleted unit.
CREATE OR REPLACE FUNCTION catalog.uom_conversions_name_live_units() RETURNS trigger AS $$
BEGIN
    IF (NEW.metadata->>'deleted_at') IS NOT NULL THEN
        RETURN NEW;
    END IF;
    PERFORM 1 FROM catalog.uoms WHERE id = NEW.from_uom_id AND (metadata->>'deleted_at') IS NULL FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'uom_not_found: the unit a conversion converts from is deleted or missing'
            USING ERRCODE = 'check_violation', CONSTRAINT = 'ck_uom_conversions_uoms_exist__uom_not_found', COLUMN = 'from_uom_id';
    END IF;
    PERFORM 1 FROM catalog.uoms WHERE id = NEW.to_uom_id AND (metadata->>'deleted_at') IS NULL FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'uom_not_found: the unit a conversion converts to is deleted or missing'
            USING ERRCODE = 'check_violation', CONSTRAINT = 'ck_uom_conversions_uoms_exist__uom_not_found', COLUMN = 'to_uom_id';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS uom_conversions_name_live_units ON catalog.uom_conversions;
CREATE TRIGGER uom_conversions_name_live_units
    BEFORE INSERT OR UPDATE OF from_uom_id, to_uom_id, metadata ON catalog.uom_conversions
    FOR EACH ROW EXECUTE FUNCTION catalog.uom_conversions_name_live_units();
