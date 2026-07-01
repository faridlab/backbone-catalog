# Catalog — Golden Cases (the oracle)

Exact expected results, mirroring the executable tests one-to-one.

## Validated writes (`tests/catalog_golden_cases.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **CGC-1** | create item group `{code, name}` | `201`; row with status `active`. |
| **CGC-2** | create item with existing group + UOM, all flags default true | `201`; item `active`, `item_type=stock`. |
| **CGC-3** | create item with a non-existent `itemGroupId` | `422 item_group_not_found`; nothing written. |
| **CGC-4** | create item with a non-existent `defaultUomId` | `422 uom_not_found`. |
| **CGC-5** | create item with all usage flags false | `422 no_usage_flag`. |
| **CGC-6** | create a second item reusing an `itemCode` | `422 duplicate_item_code`. |
| **CGC-7** | UOM conversion with `fromUomId == toUomId` | `422 same_uom`. |
| **CGC-8** | UOM conversion with `factor = 0` (or negative) | `422 non_positive_factor`. |
| **CGC-9** | UOM conversion `1 BOX = 12 PCS` (valid) | `201`; factor stored `12`. |

## Brand & product types (`tests/catalog_golden_cases.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **BGC-1** | item with a non-existent `brandId` | `422 brand_not_found`. |
| **BGC-2** | item with a valid `brandId` | `201`; brand link persisted. |
| **PGC-1** | item of type `service`/`digital_good`/`subscription`/`gift_card` with `isStockItem=true` | `201`; `is_stock_item` stored **false** (auto-derived). |
| **PGC-2** | `physical_good` with `isStockItem=true` + `tags` | `201`; `is_stock_item` true; tags persisted. |

## Variants & attributes (`tests/catalog_golden_cases.rs`)

| Case | Input | Expected |
|------|-------|----------|
| **VGC-1** | attribute value with a missing `attributeId` | `422 attribute_not_found`. |
| **VGC-2** | variant with valid options `{color:red, size:m}` | `201`; `variant_label` contains "Red" & "M"; the item's `has_variants` flips **true**. |
| **VGC-3** | variant option value not in the registry (`color:purple`) | `422 unknown_attribute_value`. |
| **VGC-4** | variant option axis not in the registry (`ghost:x`) | `422 unknown_attribute`. |
| **VGC-5** | second variant reusing a `sku` | `422 duplicate_sku`. |

## Guarded write path (`tests/integrity_probes.rs`)

| Case | Input via guarded routes | Expected |
|------|--------------------------|----------|
| **IGC-1** | `POST /items` (generic create) | not routed → `405/404` (items only via validated path). |
| **IGC-2** | `POST /items` (validated) with a missing item group | `422`. |
| **IGC-3** | `POST /uom-conversions` (validated) with `factor=0` | `422`. |
| **IGC-4** | valid item + valid conversion via guarded routes | both `201`. |
| **IGC-5** | `POST /item-variants` with an unknown option axis | `422`. |
| **IGC-6** | `DELETE`/`PATCH /uoms/:id` on the guarded surface | `405/404` (leaf-master mutation locked — ADR-005). |
| **IGC-7** | `DELETE`/`PATCH /brands/:id` on the guarded surface | `405/404`. |
| **IGC-8** | validated `POST /uoms` (+ duplicate code) | `201`, then `422 duplicate_uom_code`. |
| **IGC-9** | delete the last variant via `POST /item-variants/delete` | `200`; item's `has_variants` flips back **false**. |

## Conventions
- New Item defaults: `status=active`, `item_type=stock`, `is_sales/purchase/stock=true`, `is_taxable=true`.
- Soft-delete via `metadata->>'deleted_at'`; uniqueness (`item_code`, `barcode`, `(from,to)` UOM) is
  partial on not-deleted rows.
- A UOM conversion `factor` reads as `1 from_uom = factor to_uom`.
