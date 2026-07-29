<!--
date: 2026-07-29 | repo type: module | unit: backbone-catalog | focus: maturity
roster: chair (subagent), skeptic (subagent), steelman (subagent), yagni-business,
        ddd-bounded-context, contract-seat, domain-expert (invited — catalog encodes real domain rules)
-->

# Council — module:backbone-catalog — focus: maturity

## Verification stamp (chair, before the call)

Spot-checked the Skeptic's route claims against the files. Findings:

- `src/lib.rs:70-71` rustdoc example IS `catalog.all_crud_routes()`, but the docstring DOES label it "Unguarded full CRUD (trusted/admin); compose a guarded router for production." Not silent theatre — but it still leads with the unguarded path. Skeptic slightly overstated "unguarded, un-deprecated."
- `src/lib.rs:95-116` `all_crud_routes` mounts all 8 unguarded generic composers. CONFIRMED.
- `src/lib.rs:123` `#[deprecated]` covers ONLY `routes()`. CONFIRMED. `all_crud_routes` is un-deprecated.
- `src/routes/mod.rs:51` `create_stateless_routes` ("Use this when you only need standard CRUD"), `:83` `get_routes`, `:114` `create_combined_routes`, `:132` `get_routes_with_state` — all four mount unguarded generic CRUD, none deprecated. CONFIRMED.
- **MATERIAL CORRECTION to the Skeptic:** `src/routes/mod.rs:68` `create_readonly_catalog_routes` EXISTS — a guarded read-only base explicitly designed as the merge-parent for validated writes ("every entity mounted READ-ONLY (the guarded base)"). The Skeptic omitted it. So routes/mod.rs is not 100% unguarded; it ships one guarded base + four unguarded composers.
- `src/presentation/http/guarded_routes.rs:397` `create_guarded_catalog_routes` is real, senior-grade, and the only mount that wires `CatalogWriteService`.

Net state: **2 guarded entry points, 6 unguarded entry points, only 1 of the 6 deprecated, zero compile-time gating, zero in-tree consumers.** The Skeptic's core claim (A3 is currently UNFALSIFIABLE and the default signals point to the unguarded path) survives the correction. The "sidecar" framing is slightly too strong — there is a documented guarded base — but no compiler-enforced preference.

## Best call

**Gate the six unguarded mounts (`CatalogModule::routes`, `CatalogModule::all_crud_routes`, `create_stateless_routes`, `get_routes`, `create_combined_routes`, `get_routes_with_state`) behind `#[cfg(any(test, feature = "unguarded"))]` with the `unguarded` feature OFF by default.** Run `cargo check --no-default-features` (and `--features unguarded` against the workspace) — the resulting compile errors are the exhaustive list of every consumer that bypasses the validated write path.

This subsumes the table-states compile fix (the broken `catalog_write_service` CUSTOM block at lib.rs:213-216 must move inside the struct regardless; that is a precondition, not a recommendation).

- **Residual negative value:** ~5 min to author + the cost of touching each bypassing call site surfaced by the gate. In-tree that is currently ZERO (no sibling consumers), so today's cost is near-zero; the ongoing cost is opt-in friction for future seeders/admin tools that legitimately want unguarded CRUD (acceptable — they take an explicit, auditable action). One real risk: a sibling consumer in another repo that we cannot see would break on its next build — but that break is precisely the safety property ADR-001/002 promise, so it is value, not loss.
- **Reversibility:** **Easy.** One Cargo feature toggle flips the gate off; nothing about the validated path or the unguarded bodies changes. Fully bidirectional.
- **What would flip this:** discovery of an in-flight sibling consumer (seeder, admin tool, migration harness) that legitimately needs the unguarded path as-is AND cannot be migrated. At that point the gate is the wrong shape; the right move becomes "leave bodies public, make `create_guarded_catalog_routes` the rustdoc lead example and `#[deprecated]` the other five composers." The cheap probe (`cargo check --no-default-features` against the workspace) produces exactly this evidence before any consumer is harmed.

## Disagreement map

- **Validated path = THE path vs. sidecar option** — Skeptic says the architecture makes A3 unfalsifiable and the default signals (rustdoc example, the four unguarded composers) point the wrong way; Steelman counters with `create_readonly_catalog_routes` and `create_guarded_catalog_routes` as documented guarded bases. **Crux:** does any real consumer mount the guarded path? Cheapest decisive probe: the feature-gate compile pass above.
- **Cut the decoration vs. keep it** — YAGNI says event-sourcing (no-op publishers, 0 callers), gRPC (0 services, deps only), Specification (8 empty files), CatalogQueryService (no impl) are theatre ahead of demand; Steelman says the 5 ADRs document them as intentional. **Crux:** is there a named in-flight consumer for any of these (an event handler spec, a gRPC client contract)? None found.
- **Item is a god-entity wedge vs. intentional pragmatism** — DDD says `ItemType` + `data` JSONB + `weight_per_unit`/`shelf_life_days`/`hsn_code`/`sni_code`/`is_taxable` (schema/models/item.model.yaml) fold a second commerce/fulfillment context into neutral identity; Domain Expert + ADR-004 call it Indonesia-first pragmatism. **Crux:** does any consumer need to filter/query on the `data` bag's keys, which would force it into a real schema and expose the seam?
- **Contract is enforceable vs. porous** — Contract Seat says `pub use domain::entity::*` and `pub use infrastructure::persistence::*` (lib.rs:35,38) contradict `exports/mod.rs:5-6` and let siblings couple to `catalog::Item` / `catalog::ItemRepository` directly; Steelman says user_owned repos survive regen. **Crux:** does any sibling today depend on a type outside `exports/`?

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | **Feature-gate the six unguarded mounts default-off; compile-probe the workspace.** (Best call) | high — converts A3 from unfalsifiable to compile-enforced in one push | ~5 min + surfaced call sites (in-tree: 0); ongoing opt-in friction for legitimate admin/seeder tools | easy | A workspace consumer that legitimately needs unguarded CRUD and can't migrate |
| 2 | Move the broken `catalog_write_service` CUSTOM block inside `CatalogModule` (lib.rs:213-216). Must-fix precondition; do not ship without it | high — unblocks `cargo check` entirely | ~2 min | easy | None — it is a pure bug |
| 3 | Make `create_guarded_catalog_routes` the rustdoc lead example in `CatalogModule`; demote the `all_crud_routes` example to "admin/seeder only" | medium — closes the soft signal the Skeptic flagged | Doc drift if not owned | easy | None |
| 4 | Cut gRPC deps + empty Specification files now; keep event-sourcing + CatalogQueryService under a clear "experimental, no consumer" flag until probed | medium — removes the most misleading decoration (gRPC, Specification) without burning reversible future options | Some churn deleting tonic/prost deps; risk of re-adding later | easy for cuts; one-way for any commit history that referenced them | A consumer spec appears for either |
| 5 | Delete or implement `CatalogQueryServiceImpl` — it has no `impl CatalogQueryService for …` and is referenced nowhere (exports/services.rs:104) | low-medium — removes a lie from the public surface | Tiny | easy | A consumer wants the read contract |

## Maturity scorecard

| Seat | Axis | Score (1–5) | One sentence why |
|------|------|-------------|------------------|
| ddd-bounded-context | Bounded-context integrity | **3** | Core identity/UOM/classification is well-modeled, but `Item` carries `ItemType` + a `data` JSONB bag + commerce fields (`hsn_code`, `sni_code`, `weight_per_unit`, `shelf_life_days`, `is_taxable`) — a second context folded in via the data bag (ADR-004 acknowledges it, doesn't resolve it). |
| contract-seat | Contract minimality & enforceability | **2** | `pub use domain::entity::*` and `pub use infrastructure::persistence::*` (lib.rs:35,38) contradict `exports/mod.rs` "ONLY depend on types defined here," and the promised read contract `CatalogQueryService` has no implementation and is wired nowhere — the contract is non-minimal, unenforceable, and unfinished. |
| domain-expert | Domain invariant completeness | **3** | Validated write path correctly enforces FK existence, usage flags, factor>0, distinctness, and typed errors, but `UomConversion` is one-directional with no inverse guarantee (consumers must insert redundant rows or re-derive) and `CatalogStatus` has no transition machine (discontinued→active is a free flip; no Archived state). |

## Parking lot

- Event-sourcing layer (8 no-op publishers, EventStore/SnapshotStore, 24 documented-but-unpublished events) — raised by YAGNI, scope: root.
- gRPC deps (tonic/prost/tonic-build) with zero proto/service — raised by YAGNI, scope: root.
- Specification layer (8 empty/commented files) — raised by YAGNI, scope: root.
- CatalogQueryService read contract (finish impl or delete) — raised by Contract Seat, scope: module.
- UomConversion bidirectionality (auto-derive inverse or guard against contradictory canonical rows) — raised by Domain Expert, scope: module.
- CatalogStatus transition machine (discontinued→active gate; add Archived) — raised by Domain Expert, scope: module.
- Item god-entity split (extract commerce/fulfillment-kind to a sibling context) — raised by DDD, scope: root.
- RLS deployment verification (migration armed in deployed DB AND app role is non-superuser) — raised by Steelman C3, scope: ops/deploy.
- CI integration tests vs live PG (verify ADR-0010 fail-closed path runs in CI, not just locally) — raised by Steelman C4, scope: CI.

---

## Relevant file paths

- Compile breakage (must-fix precondition): `src/lib.rs:213-216` (orphaned `catalog_write_service` field at module top level; struct definition at `:73-82`).
- Best-call targets (gate these six): `src/lib.rs:95` (`all_crud_routes`), `:124` (`routes`); `src/routes/mod.rs:51` (`create_stateless_routes`), `:83` (`get_routes`), `:114` (`create_combined_routes`), `:132` (`get_routes_with_state`).
- Guarded paths that already exist (don't gate these): `src/presentation/http/guarded_routes.rs:397` (`create_guarded_catalog_routes`); `src/routes/mod.rs:68` (`create_readonly_catalog_routes` — the one the Skeptic omitted).
- Contract leakage: `src/lib.rs:35,38`; `src/exports/mod.rs:5-6`; `src/exports/services.rs:104` (unfinished `CatalogQueryServiceImpl`).
- God-entity seam: `schema/models/item.model.yaml` (`ItemType`, `data`, commerce fields).
- RLS migration (uncommitted, deployment-unverified): `migrations/20260722000002_enable_company_rls.up.sql`.
