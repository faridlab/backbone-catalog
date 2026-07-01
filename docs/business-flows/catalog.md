# Business Flow — Catalog (Item / ItemGroup / UOM)

> Owning module: `backbone-catalog` · Implemented in
> `src/application/service/catalog_write_service.rs`, enforced by
> `src/presentation/http/guarded_routes.rs`, proven by `tests/catalog_golden_cases.rs` and
> `tests/integrity_probes.rs`. Rules: R1–R9 in `schema/hooks/catalog.hook.yaml`.

Catalog owns the **canonical product/service identity** every other module references. It holds
identity, classification, and units — **not** prices, stock levels, or tax rules. Consuming
contexts hold their own projections (`SellableProduct`, `PurchasableItem`, `StockItem`) via a
logical FK to `catalog.Item.id`.

## Actors
- **Catalog admin / merchandiser** — defines categories, units, and items.

## Flows

### Create an item group (category)
- `POST /item-groups` `{ code, name, parentId?, isGroup? }`.
- Rule R6: `parentId` (if present) must exist → else `parent_not_found`. Unique `code` → else
  `duplicate_item_code`. → `201 { id }`.

### Create a unit of measure / brand
- **Uom** and **Brand** are leaf masters with full generic CRUD on the guarded surface (unique
  `code` is DB-enforced; no cross-entity invariant). `POST /uoms`, `POST /brands`
  `{ code, name, logoUrl?, sortOrder? }`. `Item.brand_id` is validated to exist on item create
  (rule R14 → `brand_not_found`). SEO/slug is not here — see the future backbone-seo module.

### Create an item (the load-bearing flow)
- `POST /items` `{ itemCode, name, itemGroupId, defaultUomId, itemType?, brandId?, isSalesItem?,
  isPurchaseItem?, isStockItem?, barcode?, hsnCode?, sni?, isTaxable?, tags?, data?, ... }`.
- `itemType` is the **sell-anything** kind (physical_good default / digital_good / service /
  subscription / bundle / gift_card / rental). Non-physical types are auto non-stockable
  (`is_stock_item` forced false, rule R15). Type-specific bits go in `data` (e.g.
  `{"durationMinutes":90}` for a service); `tags` is a search array.
- Rules: R1 item group exists (`item_group_not_found`); R2 default UOM exists (`uom_not_found`);
  R3 at least one usage flag true (`no_usage_flag`); R4 unique item code (`duplicate_item_code`);
  R5 unique barcode when present (`duplicate_barcode`); R14 brand exists if given
  (`brand_not_found`). → `201 { id }`.

### Create a UOM conversion
- `POST /uom-conversions` `{ fromUomId, toUomId, factor }` — means `1 fromUom = factor toUom`.
- Rules: R7 `fromUomId != toUomId` (`same_uom`); R8 `factor > 0` (`non_positive_factor`);
  R9 both UOMs exist (`uom_not_found`); unique `(from,to)` → `duplicate_conversion`. → `201 { id }`.

### Define attributes and item variants (see ADR-003)
- **Attribute** (axis) — `POST /attributes` `{ code, name, attributeType? }` (color/size/material/…).
- **AttributeValue** (option) — `POST /attribute-values` `{ attributeId, code, label, labelEn?,
  swatchHex?, sortOrder? }`. Rule R10: attribute must exist. Carries storefront presentation
  (swatch, i18n label).
- **ItemVariant** (sellable SKU) — `POST /item-variants`
  `{ itemId, sku, options:{color:"red",size:"m"}, variantLabel?, isDefault?, barcode?, weightPerUnit? }`.
  Rules: R11 item exists; R12 unique sku; R13 every option resolves to a real Attribute + value
  (`unknown_attribute` / `unknown_attribute_value`). On success the service builds `variantLabel`
  from the value labels (e.g. "Red / M") and flips `Item.has_variants = true`, in one transaction.
  Commerce (price/stock/media) is **not** here — a POS/ecommerce module projects the variant SKU.

## Postconditions
- The generated generic CRUD create/patch/upsert/bulk for Item/ItemGroup/UomConversion/Attribute/
  AttributeValue/ItemVariant is **not** mounted on the guarded surface; those rows are created only
  through the validated path.

## Not yet (parking lot)
- Item variants/templates (attribute matrix), per-item UOM conversions, bundles/kits, price lists
  (pricing lives in `backbone-promo`), item images/attachments (`backbone-bucket`), full
  category-tree cycle guard beyond direct parent existence.

See exact expected values in [golden-cases.md](golden-cases.md).
