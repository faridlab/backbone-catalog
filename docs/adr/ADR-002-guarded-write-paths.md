# ADR-002: Guarded write paths for catalog master data

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner)
**Related**: ADR-001 (catalog boundary); backbone-organization ADR-002 and backbone-accounting
ADR-002 (same finding class); council precedent `docs/council/2026-07-01-module-organization-maturity.md`

## Context

The generated 12-endpoint CRUD is mounted wide-open by `CatalogModule::routes()`, backed by generic
services with **no domain validation** (the `validation` cargo feature is off by default, so
`#[validate]` is inert; only serde required-field deserialization gates writes). A well-formed
request could therefore create:
- an Item whose `item_group_id` or `default_uom_id` does not exist,
- an Item with no usage flag (neither sellable, purchasable, nor stocked),
- a UomConversion with `from == to` or a non-positive factor.

Catalog is Tier-0 identity that every other module projects, so a corrupt row propagates widely.
This is the same class of hole the organization and accounting councils found.

## Decision

Adopt the proven guarded-composition pattern via `create_guarded_catalog_routes`:
- **Item / ItemGroup / UomConversion** — READ + **validated create** through a hand-authored
  `CatalogWriteService` (FK existence, usage flag, distinct/positive UOM conversion, uniqueness).
  Generic update/delete/upsert/bulk are not mounted on the guarded surface.
- **Uom** — full generic CRUD; a leaf master with no cross-entity invariant (unique code is
  DB-enforced), safe to expose directly.

The unguarded `routes()` remains for trusted/admin/seeding contexts; the extension guide names the
guarded composition as the default.

### Accepted trade-off
The validated writers are hand-rolled outside `BackboneCrudHandler` (flagged as an anti-pattern in
the module CLAUDE.md) — a deliberate exception, recorded here, both files `user_owned`.

## Consequences

- The invariant holes are closed on every mounted write path; four route-level integrity probes
  (`tests/integrity_probes.rs`) lock the behavior against regression.
- Residual (accepted v1 debt): no HS-code/`is_taxable` cross-check, no full category-tree cycle
  guard beyond parent existence, no variants/bundles. See ADR-001 parking lot.
