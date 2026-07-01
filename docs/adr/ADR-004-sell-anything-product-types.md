# ADR-004: "Sell anything" product typing

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner)
**Related**: ADR-001 (catalog boundary), ADR-003 (variants); learned from the old
`monorepo-backbone` bersihir Product module.

## Context

Catalog should let a seller list **anything**, not just physical goods — digital products,
services, subscriptions, gift cards, rentals. Two reference models were reviewed:

- **bersihir `Product`** (old monorepo): its `ProductType` is `raw_material / finished_good /
  consumable / spare_part / packaging` — an **inventory material classification**, and the entity
  is heavy with costing (`costing_method`, `standard_cost`, `average_cost`), reorder settings
  (`min_stock`, `reorder_quantity`, `safety_stock`, `lead_time_days`), batch/expiry tracking, and
  laundry usage metrics.
- **Our previous catalog `ItemType`** was `stock / non_stock / service / fixed_asset / template` —
  also inventory-flavored.

Neither modeled the **commerce/fulfillment kind** a marketplace needs. And most of bersihir's rich
fields (costing, reorder, batch) are inventory/accounting concerns that, in our decomposition,
belong to `backbone-inventory` / `backbone-accounting`, not catalog identity.

## Decision

1. **`ItemType` now describes the commerce / fulfillment kind** ("sell anything"):
   `physical_good` (default), `digital_good`, `service`, `subscription`, `bundle`, `gift_card`,
   `rental`. Dropped `template` (that is the `has_variants` flag) and `fixed_asset` (assets module),
   and the inventory `stock/non_stock` distinction (that is the `is_stock_item` flag).
2. **Stockability is derived from type, not trusted from the caller.** Non-physical types
   (`digital_good`, `service`, `subscription`, `gift_card`) are forced `is_stock_item = false` in
   `CatalogWriteService::create_item` (`is_physical_item_type()` = physical_good/bundle/rental).
   A service can't accidentally become stockable.
3. **Heterogeneous types carry their own bits in JSONB, not new tables.** Added `Item.tags` (search,
   `[]`) and `Item.data` (type-specific config `{}`, e.g. `{duration_minutes}` for a service,
   `{license}` for a digital good, `{billing_period}` for a subscription). Adopted from bersihir's
   `tags`/`data` and VINSTEKNIK's `data`. This keeps "sell anything" flexible without a table per
   type. (`data` is internal config, NOT publishing/SEO — that is the backbone-seo overlay.)
4. **Bersihir's inventory/costing fields are NOT adopted here** — `costing_method`, `standard_cost`,
   `average_cost`, reorder points, batch/expiry, UOM ratios (we have `UomConversion`), stock levels.
   They belong to `backbone-inventory` / `backbone-accounting`, which project `catalog.Item.id`.

## Consequences

- One `Item` can represent a drill (physical_good), a downloadable manual (digital_good), an
  installation visit (service), a maintenance plan (subscription), or a store voucher (gift_card) —
  all through the same identity, with fulfillment behavior derived from the type.
- Downstream modules read `item_type` to decide fulfillment: inventory only stocks physical types;
  a digital-delivery module handles `digital_good`; a scheduling module handles `service`; billing
  handles `subscription` recurrence.
- Residual (parking lot): `bundle` contents (an `ItemBundleComponent` table) and `rental` period
  terms are deferred until a consumer needs them; `data` is unvalidated per-type for now.
