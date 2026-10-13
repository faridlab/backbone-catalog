-- Down: drop the declared foreign keys for catalog module
ALTER TABLE IF EXISTS catalog.uom_conversions DROP CONSTRAINT IF EXISTS fk_uom_conversions_to_uom_id;
ALTER TABLE IF EXISTS catalog.uom_conversions DROP CONSTRAINT IF EXISTS fk_uom_conversions_from_uom_id;
ALTER TABLE IF EXISTS catalog.item_variants DROP CONSTRAINT IF EXISTS fk_item_variants_item_id;
ALTER TABLE IF EXISTS catalog.items DROP CONSTRAINT IF EXISTS fk_items_default_uom_id;
ALTER TABLE IF EXISTS catalog.items DROP CONSTRAINT IF EXISTS fk_items_item_group_id;
ALTER TABLE IF EXISTS catalog.attribute_values DROP CONSTRAINT IF EXISTS fk_attribute_values_attribute_id;
