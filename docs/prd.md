# backbone-catalog — PRD

> Tier-0 master-data module. Owns the **canonical product/service identity** the whole ERP
> references: **Item**, **ItemGroup** (category tree), **Uom** + **UomConversion**. Indonesia-first.

## Problem

Selling, buying, inventory, manufacturing, and billing all need a single stable answer to "what is
this product?" In ERPNext this is the `Item` god-entity — one doctype carrying sales, purchase,
stock, manufacturing, and tax config, coupling every domain to it. We need a small module that owns
**only product identity + classification + units**, so each consuming context can hold its own
projection (SellableProduct, PurchasableItem, StockItem) via a logical FK without inheriting a
god-entity.

## Scope

**In:**
- `Item` — canonical identity: `item_code` (SKU, unique), name, barcode, `brand_id`, `item_group_id`,
  `default_uom_id`, `item_type` (**sell anything**: physical_good/digital_good/service/subscription/
  bundle/gift_card/rental), usage flags (`is_sales_item`/`is_purchase_item`/`is_stock_item`, stock
  auto-derived from type), `hsn_code`/`sni` + `is_taxable` (Indonesia classification, structure
  only), `tags`/`data` JSONB for heterogeneous types, optional physical attrs.
- `ItemGroup` — category tree (parent/is_group/level).
- `Uom` — units (PCS/KG/BOX…) with `uom_type` + decimal precision.
- `UomConversion` — global `1 from = factor to` conversions.
- Validated create path + guarded routes (the invariant-bearing writes).

**Out (owned elsewhere / deferred):**
- **Prices / price lists / pricing rules** — `backbone-promo`.
- **Stock levels / warehouses / batches** — `backbone-inventory` (holds StockItem projection).
- **Sales/purchase config** (default supplier, sales tax template) — the selling/buying projections.
- **Tax rates & e-Faktur mechanics** — `backbone-tax-id` overlay. Catalog stores `hsn_code` +
  `is_taxable` only.
- **Item images/attachments** — `backbone-bucket`.
- Variants/templates matrix, bundles/kits — parking lot.

## Personas
- **Catalog admin / merchandiser** — defines categories, units, items.
- **Consuming modules** — project `catalog.Item.id` as their own product view.

## Success criteria
- One canonical item identity; consumers reference `item_id` and never re-own the god-entity.
- Every item has a valid group + default UOM and at least one usage; enforced on every write path.
- Zero horizontal Cargo edges; cross-module references are logical FKs only.
- Schema is SSoT; dedicated `catalog` Postgres schema; CRUD + migrations reproduce with no drift.

## Indonesia-first notes
- `hsn_code` (HS / kode barang) for customs & tax classification; `is_taxable` flag for PPN
  applicability. Rates/mechanics are a separate overlay — this module holds classification only.
