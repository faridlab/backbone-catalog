# Catalog Workflows

Catalog has **no multi-step sagas** — every write is a single-entity validated create
(`create_item`, `create_item_group`, `create_uom_conversion` in
`src/application/service/catalog_write_service.rs`). Nothing must span multiple entities in one
transaction.

Single-entity status transitions (Item/ItemGroup `active → inactive → discontinued`) are declared
as **state machines** in `schema/hooks/catalog.hook.yaml`, not as workflows.

Add a `*.workflow.yaml` here only if a real saga appears (e.g. "import a price list", "bundle/kit
explosion") that touches several entities atomically.
