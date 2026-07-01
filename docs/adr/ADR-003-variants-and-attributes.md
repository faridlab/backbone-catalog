# ADR-003: Variants & attributes (adopted from VINSTEKNIK, kept identity-only)

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner)
**Related**: ADR-001 (catalog boundary); learned from the VINSTEKNIK ecommerce project
(`faridlab/salt-laravel-product`)

## Context

Catalog needs to back real POS and ecommerce, which require product **variants** (a T-shirt in
Red/M vs Blue/L, each a distinct SKU) and **attributes** (Color, Size) with storefront-grade
presentation (color swatch, localized labels). We studied a production ecommerce codebase
(VINSTEKNIK, Laravel `salt-laravel-product`) to learn what a real product model needs.

### What VINSTEKNIK does (and its lesson)

- It first built a **fully-normalized** variant model — `variants` (axis) + `variant_units`
  (values) + per-product `product_variants` + `product_variant_units` (4 tables) — and **abandoned
  it**. Its current, active model is a **flat "variant-as-row"**: `products.has_variants` + a
  `product_variant_items` table where each purchasable SKU is one row with a free-form `variant`
  label string, its own `sku`, and (in their app) its own price/discount/stock/weight/dimensions.
- Option values carried useful **presentation metadata**: `label`, `label_english`, `hex`, `icon`,
  `is_primary`, `sort_order`.
- The product/variant rows **mixed identity with commerce** (price, discount, stock, wholesale,
  preorder all on the same row).

The lesson: **don't over-normalize variants**, but keep the presentation metadata a storefront/POS
needs.

## Decision

Adopt VINSTEKNIK's flat pragmatism, add just enough structure for faceting, and — per ADR-001 —
keep **identity only** (commerce stays in promo/inventory/projections):

1. **`Item.has_variants`** flag (adopted directly). When true, the purchasable units are the item's
   `ItemVariant` rows; when false, the Item itself is the unit.
2. **`ItemVariant`** = the flat variant-as-row: `item_id`, own unique `sku`, denormalized
   `variant_label` ("Red / M"), `is_default`, optional `barcode`/`weight`, and an **`options`
   JSONB** map `{attribute_code: value_code}`. JSONB + GIN keeps faceted queries ("all red")
   without a join table — leaner than the 4-table model VINSTEKNIK dropped.
3. **`Attribute` + `AttributeValue`** = a small reusable registry for the option axes and their
   values, carrying the presentation metadata we adopted (`swatch_hex`, `icon`, `label`,
   `label_en`, `sort_order`). Reusable across items (global), unlike VINSTEKNIK's per-product copies.
4. **Validated, not just declarative.** `CatalogWriteService::create_item_variant` verifies the item
   exists, the SKU is unique, and **every option key/value resolves to a real Attribute/AttributeValue**
   before persisting — so the JSONB stays referentially sound. It also builds `variant_label` from
   the value labels and flips `has_variants` in one transaction.
5. **Dropped from catalog (kept in other modules):** per-variant price/discount/currency/wholesale
   → promo; stock → inventory; images → bucket; slug/SEO/featured/condition → the selling/ecommerce
   projection. Catalog stores the SKU + option combination, not its commerce.

## Consequences

- **3 new entities** (Attribute, AttributeValue, ItemVariant) + 2 Item fields (`has_variants`,
  `sni`) — not the 4-table normalized model, and not a bare string label. A middle that stays lean
  while supporting swatches, i18n labels, and faceted queries.
- An ecommerce/POS module projects `catalog.ItemVariant.id` (the SKU) and adds price/stock/media in
  its own tables — the reuse ADR-001 promised, now variant-aware.
- Residual (parking lot): per-item allowed-attribute constraints (which axes a given item uses),
  variant image ordering, bundles/kits, and multi-value attributes per axis.
