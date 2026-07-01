# backbone-catalog — Extension Guide

How a consuming service composes and extends this module. Follows the workspace Extension Contract
(`docs/erp/extension-contract.md`).

## Composing into a service

```rust
use backbone_catalog::{CatalogModule, create_guarded_catalog_routes};

let catalog = CatalogModule::builder().with_database(pool.clone()).build()?;

// RECOMMENDED: guarded composition.
// - Item / ItemGroup / UomConversion / Attribute / AttributeValue / ItemVariant / Uom / Brand:
//   read + validated create only (no generic patch/delete/upsert/bulk on the guarded surface).
// - ItemVariant delete: POST /item-variants/delete (keeps Item.has_variants honest).
let app = axum::Router::new().merge(create_guarded_catalog_routes(&catalog));
```

Three mounts, safest to widest:

| Function | All entity writes | Use for |
|----------|-------------------|---------|
| `create_guarded_catalog_routes` | validated create only; **no generic patch/delete/upsert** on any entity (incl. Uom/Brand) | **default / production** |
| `CatalogModule::routes()` | open generic CRUD on every entity | trusted/admin, seeding only |

The wide mount (`routes()`) exposes unvalidated generated CRUD — a well-formed request can create an
item with a missing group/UOM, no usage flag, a bad UOM conversion, **or soft-delete a Uom/Brand out
from under items that reference it** (a soft delete doesn't trip the FK, but every read filters it
out). Use it only behind auth in trusted contexts. See [ADR-002](adr/ADR-002-guarded-write-paths.md)
and [ADR-005](adr/ADR-005-leaf-master-write-lock.md).

`catalog.catalog_write_service` is exposed to call the validated operations directly (e.g. seeders).

## Public / stable surface
- **Entities & DTOs** — Item/ItemGroup/Uom/UomConversion + generated DTOs.
- **Validated write API** — `CatalogWriteService`, `NewItem`/`NewItemGroup`/`NewUomConversion`,
  `CatalogWriteError`, and `create_guarded_catalog_routes`.
- **Logical FK identity** — `Item.id`, `ItemGroup.id`, `Uom.id`. Reference these from other
  contexts as `item_id` etc.; never add a DB FK across the module boundary. Hold your own projection
  (SellableProduct/PurchasableItem/StockItem) — see [ADR-001](adr/ADR-001-catalog-boundary.md).

## Regeneration safety
Hand-authored files are `user_owned` in `metaphor.codegen.yaml` and survive
`metaphor schema schema generate --force`:
`src/application/service/catalog_write_service.rs`, `src/presentation/http/guarded_routes.rs`,
`tests/catalog_golden_cases.rs`, `tests/integrity_probes.rs`, `tests/features/**`, `docs/**`.
The module gets its own `catalog` Postgres schema (`schema: catalog` in `index.model.yaml`).
