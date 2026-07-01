# backbone-catalog — FSD

Schema (`schema/models/*.model.yaml`) is the SSoT; this documents behavior and integration the
schema does not encode.

## Entities

| Entity | Table | Key identity | Notes |
|--------|-------|--------------|-------|
| ItemGroup | `catalog.item_groups` | `code` unique | Category tree (`parent_id`, `is_group`, `level`). |
| Uom | `catalog.uoms` | `code` unique | `uom_type`, `decimal_places`. |
| Brand | `catalog.brands` | `code` unique | Merek; `logo_url`, `sort_order`. Leaf master. |
| Item | `catalog.items` | `item_code` unique; `barcode` unique when present | FK `item_group_id`, `default_uom_id`, `brand_id?`; `item_type` (sell-anything); usage flags; `hsn_code`/`sni`/`is_taxable`; `has_variants`; `tags`/`data` JSONB. |
| UomConversion | `catalog.uom_conversions` | `(from_uom_id,to_uom_id)` unique | `factor` (`1 from = factor to`), `> 0`. |
| Attribute | `catalog.attributes` | `code` unique | Reusable variant axis; `attribute_type`. |
| AttributeValue | `catalog.attribute_values` | `(attribute_id,code)` unique | Option value; `label`/`label_en`/`swatch_hex`/`icon`/`sort_order`. |
| ItemVariant | `catalog.item_variants` | `sku` unique; `barcode` unique when present | FK `item_id`; `variant_label`; `options` JSONB `{attr_code:value_code}`; `is_default`. See ADR-003. |

Tables live in the **`catalog` Postgres schema**. Soft-delete via `metadata->>'deleted_at'`; all
uniqueness is partial on not-deleted rows.

## Endpoints

- **Generated CRUD** — 12 Backbone endpoints per entity (mounted by `CatalogModule::routes()`).
- **Guarded surface (recommended)** — `create_guarded_catalog_routes(&CatalogModule)`:
  - Item / ItemGroup / UomConversion: **read + validated create** (`POST /items`,
    `POST /item-groups`, `POST /uom-conversions`). Generic update/delete/upsert/bulk not mounted.
  - Uom: full generic CRUD (leaf master; unique code DB-enforced).

## Validated write rules (R1–R9)

See `schema/hooks/catalog.hook.yaml`. In short: item group + default UOM must exist; item needs at
least one usage flag; item_code/barcode unique; item-group parent must exist; UOM conversions must
have distinct UOMs, a positive factor, and existing UOMs. Error codes: `item_group_not_found`,
`uom_not_found`, `no_usage_flag`, `duplicate_item_code`, `duplicate_barcode`, `parent_not_found`,
`same_uom`, `non_positive_factor`, `duplicate_conversion` (all `422`).

## Integration points (logical FKs — no DB FK, no Cargo edge)

- Downstream modules reference `catalog.Item.id` (`item_id`), `catalog.Uom.id`, `catalog.ItemGroup.id`
  and hold their own projection. Catalog never imports a consuming module.

## Behavior specs
- Hooks: `schema/hooks/catalog.hook.yaml` (state machines + R1–R9 + events).
- Workflows: none (no multi-step saga) — see `schema/workflows/README.md`.
- Business flows + oracle: `docs/business-flows/` + `tests/features/catalog.feature`; executable
  oracle `tests/catalog_golden_cases.rs` + `tests/integrity_probes.rs`.

## Product types ("sell anything")
`item_type` is the commerce/fulfillment kind: `physical_good` (default), `digital_good`, `service`,
`subscription`, `bundle`, `gift_card`, `rental` (see ADR-004). Non-physical types
(digital/service/subscription/gift_card) are **auto non-stockable** — `create_item` forces
`is_stock_item = false` for them. Type-specific bits live in the `data` JSONB (e.g.
`{duration_minutes}` for a service); `tags` is a search array. Inventory/costing (stock levels,
costing method, reorder, batch/expiry) is **not** here — it lives in `backbone-inventory`/
`backbone-accounting`, which project `catalog.Item.id`.

## SEO / publishing — deliberately NOT here
Catalog has **no `slug`/`seo` fields**. SEO and public-URL/publishing are a cross-cutting overlay
concern (like tax and localization), and not every item is published to a public channel. A future
**backbone-seo** module will own a polymorphic `SeoMeta` (target module + id logical FK, per-channel
slug/meta/og/canonical/robots) that only publishable entities reference. See
`docs/erp/vinsteknik-adoption-map.md`.

## Non-goals in code
No prices, stock levels, tax rates, item images, or SEO/slug. See [prd.md](prd.md) "Out".
