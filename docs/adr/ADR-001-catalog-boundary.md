# ADR-001: The catalog bounded context (identity, not god-entity)

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner)
**Related**: workspace `docs/erp/shared-masters-ownership.md`, `docs/erp/module-map.md`

## Context

ERPNext's `Item` is a god-entity: one DocType carrying sales, purchase, stock, manufacturing, and
tax configuration, which couples every domain to it. The decomposition needs a Tier-0 module that
owns product identity without becoming that god-entity.

## Decision

1. **`backbone-catalog` owns only identity + classification + units** — Item, ItemGroup, Uom,
   UomConversion. It does **not** own prices, stock levels, sales/purchase config, tax rates, or
   images.
2. **Per-context projections, not shared config.** Each consuming context holds its own view of an
   item via a logical FK to `catalog.Item.id`: `SellableProduct` (selling), `PurchasableItem`
   (buying), `StockItem` (inventory). Usage flags (`is_sales_item`/`is_purchase_item`/
   `is_stock_item`) declare which projections are allowed; the projection data lives in the
   consumer, not here.
3. **Dedicated Postgres schema** (`schema: catalog`) — like every backbone module.
4. **Indonesia-first classification only.** `hsn_code` + `is_taxable` are stored as neutral
   classification; PPN rates and e-Faktur mechanics are deferred to `backbone-tax-id`.
5. **Cross-module references are logical FKs only** — no DB foreign keys, no Cargo edges.

## Consequences

- Consumers depend on a small, stable identity contract (a UUID + a few classification fields),
  not on a wide god-entity that changes whenever any one domain changes.
- The module is thin: master data + one validated write path. No GL posting, no pricing, no stock.
- Adding a new consuming domain means adding a projection in that module, never widening Item.
- Variants & attributes are now supported (see ADR-003) — identity-only, commerce stays external.
- Parking lot: per-item UOM conversions, bundles/kits, price lists, per-item allowed-attribute sets.
