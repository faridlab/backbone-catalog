//! Golden cases for the UoM parent-store tree (ADR-0023).
//!
//! The wave-definition named cases, proven against real Postgres:
//!   1. multi-level factor recursion (box -> pack -> unit),
//!   2. direct (stored-factor) and derived (hop-by-hop) conversions agreeing,
//!   3. cross-tree conversion refused LOUDLY with a typed error naming both trees,
//!   4. rounding-policy obedience (the caller declares the policy; the converter never
//!      guesses from a precision label),
//!   5. idempotent re-run of the stored-factor backfill/recompute (including healing a
//!      corrupted chain),
//!   plus the tree write-path guards (shape, positivity, cycle, re-parent re-derivation).
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433). Tenancy: none, by
//! design (ADR-0029) — every test runs plain on the pool, undecorated; per-unit tree
//! scoping is the composing service's decorator's job, not the module's.

use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::{
    CatalogWriteError, CatalogWriteService, ConversionRounding, NewItem, NewItemGroup, NewUom,
    UomConversionError,
};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string()
    });
    PgPool::connect(&url).await.unwrap()
}

fn uq(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn new_uom(
    svc: &CatalogWriteService,
    code: &str,
    relative: Option<(Uuid, Decimal)>,
) -> Uuid {
    let (relative_uom_id, relative_factor) = match relative {
        None => (None, None),
        Some((p, rf)) => (Some(p), Some(rf)),
    };
    svc.create_uom(NewUom {
        code: uq(code),
        name: code.to_string(),
        uom_type: None,
        decimal_places: 0,
        relative_uom_id,
        relative_factor,
    })
    .await
    .expect("create uom")
}

async fn factor_of(pool: &PgPool, id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT factor FROM catalog.uoms WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The canonical three-level chain: UNIT (root) <- PACK (x10) <- BOX (x12).
/// Stored factors must be 1 / 10 / 120 — the recursive product of the chain.
struct Chain {
    unit: Uuid,
    pack: Uuid,
    box_: Uuid,
}

async fn seed_box_pack_unit(svc: &CatalogWriteService) -> Chain {
    let unit = new_uom(svc, "UNIT", None).await;
    let pack = new_uom(svc, "PACK", Some((unit, Decimal::from(10)))).await;
    let box_ = new_uom(svc, "BOX", Some((pack, Decimal::from(12)))).await;
    Chain { unit, pack, box_ }
}

// GC-UOM-1: multi-level factor recursion — the stored factor is the product of every
// relative_factor link down to the root, and roots store 1.
#[tokio::test]
async fn multi_level_factor_recursion() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;
    assert_eq!(factor_of(&pool, c.unit).await, Decimal::from(1));
    assert_eq!(factor_of(&pool, c.pack).await, Decimal::from(10));
    assert_eq!(factor_of(&pool, c.box_).await, Decimal::from(120));
}

// GC-UOM-2: direct and derived conversions agree.
//   direct  = one step over the STORED factors (what convert_quantity does),
//   derived = walking the chain hop by hop (box -> pack -> unit) in the test,
// plus the stored factor equals the product of the loaded relative_factor links
// (UomChain::derived_root_factor) for every level of the chain.
#[tokio::test]
async fn direct_and_derived_conversions_agree() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;
    let qty = Decimal::from(7);

    // Direct: 7 BOX -> UNIT in one step over stored factors.
    let direct = svc
        .convert_quantity(qty, c.box_, c.unit, ConversionRounding::Exact)
        .await
        .expect("box -> unit");
    assert_eq!(direct, Decimal::from(7) * Decimal::from(120));

    // Derived: 7 BOX -> PACK (ratio 12), then PACK -> UNIT (ratio 10).
    let box_to_pack = svc
        .convert_quantity(qty, c.box_, c.pack, ConversionRounding::Exact)
        .await
        .expect("box -> pack");
    assert_eq!(box_to_pack, Decimal::from(7) * Decimal::from(12));
    let derived = svc
        .convert_quantity(box_to_pack, c.pack, c.unit, ConversionRounding::Exact)
        .await
        .expect("pack -> unit");
    assert_eq!(direct, derived, "one-step and hop-by-hop conversions must agree");

    // The stored factors equal the walked products for every level.
    let uoms =
        backbone_catalog::infrastructure::persistence::UomRepository::new(pool.clone());
    for (leaf, expected) in [
        (c.unit, Decimal::from(1)),
        (c.pack, Decimal::from(10)),
        (c.box_, Decimal::from(120)),
    ] {
        let rows = uoms.load_tree_chain(&pool, leaf).await.unwrap().unwrap();
        let chain = backbone_catalog::UomChain::from_rows(leaf, rows).unwrap();
        assert_eq!(chain.leaf().factor, expected, "stored factor for {expected:?}");
        assert_eq!(chain.derived_root_factor().unwrap(), expected, "walked factor");
    }
}

// GC-UOM-3: cross-tree conversion fails LOUDLY — a typed error naming both trees,
// never a silent numeric result (the ADR-0023 replacement for multiply-by-nonsense).
#[tokio::test]
async fn cross_tree_conversion_refuses_loudly() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;
    // A second, unrelated tree: KG root with a G child.
    let kg = new_uom(&svc, "KG", None).await;
    let g = new_uom(&svc, "G", Some((kg, Decimal::new(1, 3)))).await; // 1 g = 0.001 kg

    for target in [kg, g] {
        let err = svc
            .convert_quantity(Decimal::from(5), c.box_, target, ConversionRounding::Exact)
            .await
            .expect_err("cross-tree conversion must fail");
        match &err {
            CatalogWriteError::Conversion(UomConversionError::CrossTree {
                from_code,
                from_root_code,
                to_code,
                to_root_code,
            }) => {
                // The error names BOTH trees by their roots (codes carry the unique suffix).
                assert!(from_code.starts_with("BOX-"), "{from_code}");
                assert!(from_root_code.starts_with("UNIT-"), "{from_root_code}");
                assert!(to_code.starts_with("KG-") || to_code.starts_with("G-"), "{to_code}");
                assert!(to_root_code.starts_with("KG-"), "{to_root_code}");
                let msg = err.to_string();
                assert!(msg.contains(&from_root_code[..9]), "message names the source tree: {msg}");
                assert!(msg.contains(&to_root_code[..9]), "message names the target tree: {msg}");
            }
            other => panic!("expected CrossTree, got {other:?}"),
        }
    }

    // Within the tree it converts fine both ways (inverse derivation, no factor_inv column).
    let up = svc
        .convert_quantity(Decimal::from(1200), c.unit, c.box_, ConversionRounding::Exact)
        .await
        .unwrap();
    assert_eq!(up, Decimal::from(10));
}

// GC-UOM-4: rounding-policy obedience — the caller declares the policy; different policies
// over the same exact value produce their own (correct) results, and Exact returns the
// unrounded quotient. 5 PACK = 50/120 BOX = 0.41666… — 2dp half-away-from-zero is 0.42,
// 2dp toward-zero is 0.41.
#[tokio::test]
async fn rounding_policy_is_obeyed() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;
    let qty = Decimal::from(5); // 5 PACK

    let exact = svc
        .convert_quantity(qty, c.pack, c.box_, ConversionRounding::Exact)
        .await
        .unwrap();
    assert_eq!(exact, Decimal::from(50) / Decimal::from(120));

    let half_up = svc
        .convert_quantity(
            qty,
            c.pack,
            c.box_,
            ConversionRounding::Places { dp: 2, strategy: RoundingStrategy::MidpointAwayFromZero },
        )
        .await
        .unwrap();
    assert_eq!(half_up, Decimal::new(42, 2)); // 0.42

    let toward_zero = svc
        .convert_quantity(
            qty,
            c.pack,
            c.box_,
            ConversionRounding::Places { dp: 2, strategy: RoundingStrategy::ToZero },
        )
        .await
        .unwrap();
    assert_eq!(toward_zero, Decimal::new(41, 2)); // 0.41

    // The policy also applies to a true midpoint on the identity conversion.
    let identity = svc
        .convert_quantity(
            Decimal::new(2345, 3), // 2.345
            c.unit,
            c.unit,
            ConversionRounding::Places { dp: 2, strategy: RoundingStrategy::MidpointAwayFromZero },
        )
        .await
        .unwrap();
    assert_eq!(identity, Decimal::new(235, 2)); // 2.35
}

// GC-UOM-5: the stored-factor recompute (the write path's backfill, same SQL shape the
// migration ran) is idempotent and heals corrupted chains.
#[tokio::test]
async fn backfill_recompute_is_idempotent_and_heals_chains() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;

    // Corrupt one stored factor directly (simulating drift the recompute must heal).
    sqlx::query("UPDATE catalog.uoms SET factor = 999 WHERE id = $1")
        .bind(c.box_)
        .execute(&pool)
        .await
        .unwrap();

    // Run the recompute twice — the second run must be a strict no-op.
    sqlx::query("SELECT catalog.uom_recompute_factors()").execute(&pool).await.unwrap();
    assert_eq!(factor_of(&pool, c.box_).await, Decimal::from(120), "chain healed");

    let chain_ids = vec![c.unit, c.pack, c.box_];
    let before: Vec<(Uuid, Decimal, Option<String>)> = sqlx::query_as(
        "SELECT id, factor, metadata->>'updated_at' FROM catalog.uoms WHERE id = ANY($1)",
    )
    .bind(&chain_ids)
    .fetch_all(&pool)
    .await
    .unwrap();
    sqlx::query("SELECT catalog.uom_recompute_factors()").execute(&pool).await.unwrap();
    sqlx::query("SELECT catalog.uom_recompute_factors()").execute(&pool).await.unwrap();
    let after: Vec<(Uuid, Decimal, Option<String>)> = sqlx::query_as(
        "SELECT id, factor, metadata->>'updated_at' FROM catalog.uoms WHERE id = ANY($1)",
    )
    .bind(&chain_ids)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(before.len(), after.len());
    let mut b: Vec<_> = before.into_iter().map(|(i, f, u)| (i, f, u)).collect();
    let mut a = after;
    b.sort_by(|x, y| x.0.cmp(&y.0));
    a.sort_by(|x, y| x.0.cmp(&y.0));
    for ((bi, bf, bu), (ai, af, au)) in b.into_iter().zip(a) {
        assert_eq!(bi, ai);
        assert_eq!(bf, af, "factors unchanged after re-run");
        assert_eq!(bu, au, "no rows rewritten on an idempotent re-run (updated_at untouched)");
    }
}

// GC-UOM-6: tree write guards — shape (both link fields together), positivity, unknown
// parent, and the descendant cycle rule; plus re-parenting re-derives the stored factors
// of the whole subtree and detaching makes a root.
#[tokio::test]
async fn tree_write_guards() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let c = seed_box_pack_unit(&svc).await;

    // Shape: parent without ratio.
    let mut u = NewUom {
        code: uq("BAD1"),
        name: "Bad".into(),
        uom_type: None,
        decimal_places: 0,
        relative_uom_id: Some(c.unit),
        relative_factor: None,
    };
    assert!(matches!(
        svc.create_uom(u.clone()).await.unwrap_err(),
        CatalogWriteError::RelativeShapeMismatch
    ));
    // Shape: ratio without parent.
    u.relative_uom_id = None;
    u.relative_factor = Some(Decimal::from(2));
    u.code = uq("BAD2");
    assert!(matches!(
        svc.create_uom(u).await.unwrap_err(),
        CatalogWriteError::RelativeShapeMismatch
    ));

    // Positivity.
    assert!(matches!(
        new_uom_err(&svc, "BAD3", Some((c.unit, Decimal::ZERO))).await,
        CatalogWriteError::NonPositiveRelativeFactor
    ));

    // Unknown parent.
    assert!(matches!(
        new_uom_err(&svc, "BAD4", Some((Uuid::new_v4(), Decimal::from(2)))).await,
        CatalogWriteError::ParentNotFound(_)
    ));

    // Cycle: pointing the ROOT at its own descendant must be refused loudly.
    assert!(matches!(
        svc.set_uom_relative(c.unit, Some((c.box_, Decimal::from(2)))).await.unwrap_err(),
        CatalogWriteError::UomCycle { .. }
    ));
    // Self-link is the same refusal.
    assert!(matches!(
        svc.set_uom_relative(c.pack, Some((c.pack, Decimal::ONE))).await.unwrap_err(),
        CatalogWriteError::UomCycle { .. }
    ));
    // Nothing was written by the refused attempts.
    assert_eq!(factor_of(&pool, c.unit).await, Decimal::from(1));

    // Re-parent changes the stored factors of the whole subtree: re-scale PACK from
    // x10 to x20 — PACK becomes 20 and BOX (x12 over PACK) becomes 240.
    svc.set_uom_relative(c.pack, Some((c.unit, Decimal::from(20)))).await.expect("rescale");
    assert_eq!(factor_of(&pool, c.pack).await, Decimal::from(20));
    assert_eq!(factor_of(&pool, c.box_).await, Decimal::from(240));

    // Detach PACK to become its own root: PACK=1, BOX=12.
    svc.set_uom_relative(c.pack, None).await.expect("detach");
    assert_eq!(factor_of(&pool, c.pack).await, Decimal::from(1));
    assert_eq!(factor_of(&pool, c.box_).await, Decimal::from(12));

    // After the detach, BOX and UNIT no longer share a root: cross-tree refusal.
    assert!(matches!(
        svc.convert_quantity(Decimal::ONE, c.box_, c.unit, ConversionRounding::Exact)
            .await
            .unwrap_err(),
        CatalogWriteError::Conversion(UomConversionError::CrossTree { .. })
    ));
}

async fn new_uom_err(
    svc: &CatalogWriteService,
    code: &str,
    relative: Option<(Uuid, Decimal)>,
) -> CatalogWriteError {
    let (relative_uom_id, relative_factor) = match relative {
        None => (None, None),
        Some((p, rf)) => (Some(p), Some(rf)),
    };
    svc.create_uom(NewUom {
        code: uq(code),
        name: code.to_string(),
        uom_type: None,
        decimal_places: 0,
        relative_uom_id,
        relative_factor,
    })
    .await
    .unwrap_err()
}

// GC-UOM-7: protected units (UM-4) — the validated retire path and its storage backstop.
//   - a user-created unit with nothing leaning on it archives (soft delete) cleanly;
//   - a module-seeded (protected) unit refuses via the service with the typed
//     ProtectedUom error;
//   - the same protected row ALSO refuses at the storage layer on both delete shapes —
//     a raw row DELETE and a raw soft-delete UPDATE each raise, so no write path
//     (generic CRUD, raw SQL) can bypass the guard the way upstream's ORM-only
//     @api.ondelete hook could;
//   - a unit that is still the live parent of live units refuses (UomHasChildren);
//   - a unit that is the default of a live item refuses (UomInUse);
//   - ordinary writes on protected rows (factor recompute, status-retire via the
//     lifecycle) still work — the guard refuses deletion only, not editing or archiving.
#[tokio::test]
async fn protected_units_delete_guards() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    // A user unit nothing leans on: retires cleanly.
    let lone = new_uom(&svc, "LONE", None).await;
    svc.delete_uom(lone).await.expect("user unit deletes");
    let archived: bool = sqlx::query_scalar(
        "SELECT (metadata->>'deleted_at') IS NOT NULL FROM catalog.uoms WHERE id = $1",
    )
    .bind(lone)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(archived, "retire is a soft delete: the row stays, stamped");

    // A module-seeded (protected) unit: the seeding path marks is_protected at
    // insert time; simulate it with a direct UPDATE (the only writer of the flag).
    let seeded = new_uom(&svc, "SEEDED", None).await;
    sqlx::query("UPDATE catalog.uoms SET is_protected = true WHERE id = $1")
        .bind(seeded)
        .execute(&pool)
        .await
        .unwrap();

    // Service path: typed refusal, never a silent archive.
    let err = svc.delete_uom(seeded).await.expect_err("protected unit refuses");
    assert!(matches!(err, CatalogWriteError::ProtectedUom { .. }));
    assert_eq!(err.code(), "protected_uom");
    let still_live: bool = sqlx::query_scalar(
        "SELECT (metadata->>'deleted_at') IS NULL FROM catalog.uoms WHERE id = $1",
    )
    .bind(seeded)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(still_live, "the refused delete wrote nothing");

    // Storage backstop, hard delete: raw SQL DELETE on the protected row raises.
    let hard = sqlx::query("DELETE FROM catalog.uoms WHERE id = $1")
        .bind(seeded)
        .execute(&pool)
        .await;
    assert!(hard.is_err(), "raw DELETE on a protected unit must raise");

    // Storage backstop, soft delete: the deleted_at-stamping UPDATE raises too.
    let soft = sqlx::query(
        "UPDATE catalog.uoms \
         SET metadata = jsonb_set(metadata, '{deleted_at}', to_jsonb(now())) \
         WHERE id = $1",
    )
    .bind(seeded)
    .execute(&pool)
    .await;
    assert!(soft.is_err(), "soft-delete UPDATE on a protected unit must raise");

    // The guard refuses deletion only: ordinary writes on the protected row work —
    // factor recompute is a no-op here but exercises the UPDATE path, and the
    // status lifecycle (the retire story for seeded units) stays writable.
    sqlx::query("SELECT catalog.uom_recompute_factors()").execute(&pool).await.unwrap();
    sqlx::query("UPDATE catalog.uoms SET status = 'inactive' WHERE id = $1")
        .bind(seeded)
        .execute(&pool)
        .await
        .unwrap();

    // A unit with live children: refuse until the subtree is re-linked.
    let parent = new_uom(&svc, "PARENT", None).await;
    let _child = new_uom(&svc, "CHILD", Some((parent, Decimal::from(10)))).await;
    let err = svc.delete_uom(parent).await.expect_err("parent refuses while children live");
    assert!(matches!(err, CatalogWriteError::UomHasChildren { .. }));
    // Detach the child; now the parent retires.
    svc.set_uom_relative(_child, None).await.expect("detach child");
    svc.delete_uom(parent).await.expect("parent deletes once childless");

    // A unit a live item defaults to: refuse (the orphaning hazard of ADR-005).
    let used = new_uom(&svc, "USED", None).await;
    let group = svc
        .create_item_group(NewItemGroup {
            code: uq("GRP"),
            name: "Group".into(),
            parent_id: None,
            is_group: false,
        })
        .await
        .expect("item group");
    svc.create_item(NewItem {
        item_code: uq("ITEM"),
        name: "Item".into(),
        description: None,
        barcode: None,
        brand_id: None,
        item_group_id: group,
        default_uom_id: used,
        item_type: None,
        is_sales_item: true,
        is_purchase_item: false,
        is_stock_item: false,
        hsn_code: None,
        is_taxable: true,
        weight_per_unit: None,
        standard_cost: None,
        tags: None,
        data: None,
    })
    .await
    .expect("item");
    let err = svc.delete_uom(used).await.expect_err("referenced unit refuses");
    assert!(matches!(err, CatalogWriteError::UomInUse { .. }));

    // Unknown unit: loud typed refusal (no silent success on a missing row).
    assert!(matches!(
        svc.delete_uom(Uuid::new_v4()).await.unwrap_err(),
        CatalogWriteError::UomNotFound(_)
    ));
}
