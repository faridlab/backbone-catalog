# ADR-005: Lock leaf-master mutation on the guarded surface

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner), council (module:backbone-catalog, focus=maturity, 2026-07-01)
**Related**: ADR-002 (guarded write paths); organization ADR-002 / accounting ADR-002 (same finding class)

## Context

A maturity-focus council found a HIGH hole on the surface explicitly labelled "guarded / recommended
for any real deployment." `create_guarded_catalog_routes` mounted **Uom** and **Brand** with the full
generated CRUD (`create_uom_routes`/`create_brand_routes` = the 16-endpoint `BackboneCrudHandler`,
incl. PATCH/PUT/DELETE/upsert), on the theory that they are "leaf masters with no cross-entity
invariant."

They are not leaves: `Item.default_uom_id`, `Item.brand_id`, and `UomConversion.from/to_uom_id`
reference them with hard DB FKs. And the generated DELETE is a **soft delete** — it stamps
`metadata->>'deleted_at'`, the row stays, so the FK never fires — while every validated read/`exists()`
filters `deleted_at IS NULL`. So a Uom or Brand could be soft-deleted (or PATCH-recoded) out from under
live items with zero error: the item's `default_uom` then resolves to nothing, and inventory/invoice/
pricing projections that JOIN on non-deleted uoms drop the row or render a blank unit.

The tell: `attribute_values` was already mounted **read-only** on the same surface (correct) — the fix
was applied to one side of the same hazard class and not the other. And no integrity probe touched the
Uom/Brand mutation path, so the hole was both real and invisible.

## Decision

1. **Uom and Brand are read-only + validated-create on the guarded surface** — mount
   `create_uom_read_routes`/`create_brand_read_routes` (GET only) plus validated
   `POST /uoms` / `POST /brands` via `CatalogWriteService::{create_uom, create_brand}`. Generic
   patch/delete/upsert/bulk are no longer mounted. This mirrors the `attribute_values` treatment.
2. **Keep `Item.has_variants` honest.** `POST /item-variants/delete` (`delete_item_variant`)
   soft-deletes a variant and, when the item has no live variants left, flips `has_variants` back to
   false — so the storefront picker can't be lied to by a stale flag.
3. **Integrity probes cover the newly-locked paths** (`tests/integrity_probes.rs`): guarded surface
   rejects DELETE/PATCH on `/uoms` and `/brands` (405/404), validated `POST /uoms` works and dedupes,
   and deleting the last variant resets `has_variants`.

## Consequences

- The orphan-the-parent hole is closed on the guarded surface, with regression probes.
- Uom/Brand can no longer be edited in place through the guarded routes. If a real consumer needs to
  rename/retire a Uom or Brand, add a **validated update** that either blocks when referenced or
  propagates the re-code — do not re-open generic mutation.
- **Resolved at the generator level (2026-07-01):** the unguarded full-CRUD mount is now emitted as
  `all_crud_routes()` (explicit, doc-warned), and `routes()` is a `#[deprecated]` alias that steers
  callers to a guarded composition — so a naive `.routes()` call warns at compile time. This applies
  to **every** module (fixed once in `metaphor-plugin-schema/src/generators/module.rs`), closing the
  same latent ergonomics gap in organization/accounting too.
- Residual (accepted, parking lot per the council): the `options` JSONB can still drift if an
  `AttributeValue` is retired after a variant references it (LOW — a dead variant code, not an orphaned
  FK); per-item allowed-attribute sets and bundle/rental terms remain deferred.
