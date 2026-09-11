//! Golden-case tests for the catalog validated write path.
//! Proves CatalogWriteService enforces the R1–R9 rules against real Postgres.
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).
//!
//! Tenancy: none, by design (ADR-0029) — the module is tenant-agnostic and runs
//! undecorated here: statements run plain on the pool with no scope bound. Case
//! isolation comes from the random suffix every seeded code carries (`uq`), and
//! from per-case rows; org scoping is the composing service's decorator's job.

use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::{
    CatalogWriteError, CatalogWriteService, NewAttribute, NewAttributeValue, NewItem,
    NewItemGroup, NewItemVariant,
};
use backbone_catalog::domain::entity::CatalogStatus;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string()
    });
    PgPool::connect(&url).await.unwrap()
}

fn uq(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn seed_uom(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_brand(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.brands (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

fn item(code: &str, group: Uuid, uom: Uuid) -> NewItem {
    NewItem {
        item_code: code.to_string(),
        name: "Item".into(),
        description: None,
        barcode: None,
        brand_id: None,
        item_group_id: group,
        default_uom_id: uom,
        item_type: None,
        is_sales_item: true,
        is_purchase_item: true,
        is_stock_item: true,
        hsn_code: None,
        is_taxable: true,
        weight_per_unit: None,
        standard_cost: None,
        tags: None,
        data: None,
    }
}

// CGC-1/2: group + item happy path
#[tokio::test]
async fn create_group_and_item() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc
        .create_item_group(NewItemGroup {
            code: uq("FG"), name: "Finished".into(), parent_id: None, is_group: false,
        })
        .await
        .expect("group");
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let id = svc.create_item(item(&uq("SKU"), gid, uom)).await.expect("item");

    let row = sqlx::query_scalar::<_, String>(
        "SELECT item_type::text FROM catalog.items WHERE id=$1",
    )
    .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, "physical_good");
}

// CGC-3: the standard_cost write→read roundtrip. A supplied cost persists exactly (it is the
// margin-math anchor selling snapshots at order-confirm time); an absent cost stays NULL —
// unknown cost, never zero.
#[tokio::test]
async fn item_standard_cost_roundtrip() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "Finished".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;

    let mut with_cost = item(&uq("SKU"), gid, uom);
    with_cost.standard_cost = Some(rust_decimal::Decimal::new(123456789012, 6)); // 123456.789012
    let id_cost = svc.create_item(with_cost).await.expect("item with cost");
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT standard_cost::text FROM catalog.items WHERE id=$1",
    )
    .bind(id_cost).fetch_one(&pool).await.unwrap();
    assert_eq!(stored.as_deref(), Some("123456.789012"), "cost must persist exactly");

    let id_null = svc.create_item(item(&uq("SKU"), gid, uom)).await.expect("item without cost");
    let stored_null: Option<String> = sqlx::query_scalar(
        "SELECT standard_cost::text FROM catalog.items WHERE id=$1",
    )
    .bind(id_null).fetch_one(&pool).await.unwrap();
    assert_eq!(stored_null, None, "absent cost must stay NULL, never default to zero");
}

// PGC-1: a non-physical type (service/digital) is auto non-stockable, even if the caller asks.
#[tokio::test]
async fn non_physical_types_are_not_stockable() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("SVC"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("UNIT")).await;
    for kind in ["service", "digital_good", "subscription", "gift_card"] {
        let mut it = item(&uq("SKU"), gid, uom);
        it.item_type = Some(kind.to_string());
        it.is_stock_item = true; // caller asks for stock…
        it.is_sales_item = true;
        it.data = Some(serde_json::json!({"note": kind}));
        let id = svc.create_item(it).await.unwrap_or_else(|e| panic!("{kind}: {e:?}"));
        let stock: bool = sqlx::query_scalar("SELECT is_stock_item FROM catalog.items WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert!(!stock, "{kind} must be non-stockable");
    }
}

// PGC-2: a physical good keeps stockability + persists tags/data.
#[tokio::test]
async fn physical_good_keeps_stock_and_tags() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.item_type = Some("physical_good".into());
    it.is_stock_item = true;
    it.tags = Some(serde_json::json!(["bor", "listrik"]));
    let id = svc.create_item(it).await.expect("item");
    let (stock, tags): (bool, serde_json::Value) =
        sqlx::query_as("SELECT is_stock_item, tags FROM catalog.items WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
    assert!(stock);
    assert_eq!(tags, serde_json::json!(["bor", "listrik"]));
}

// CGC-3: missing item group
#[tokio::test]
async fn item_rejects_missing_group() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let err = svc.create_item(item(&uq("SKU"), Uuid::new_v4(), uom)).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::ItemGroupNotFound(_)));
}

async fn status_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status::text FROM catalog.items WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// CGC-LC1: the CatalogStatus state machine allows active↔inactive and active→discontinued.
#[tokio::test]
async fn item_status_transitions_valid() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc
        .create_item_group(NewItemGroup {
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        })
        .await
        .unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let id = svc.create_item(item(&uq("SKU"), gid, uom)).await.expect("item");

    svc.transition_item_status(id, CatalogStatus::Inactive).await.expect("active->inactive");
    assert_eq!(status_of(&pool, id).await, "inactive");
    svc.transition_item_status(id, CatalogStatus::Active).await.expect("inactive->active");
    assert_eq!(status_of(&pool, id).await, "active");
    svc.transition_item_status(id, CatalogStatus::Discontinued).await.expect("active->discontinued");
    assert_eq!(status_of(&pool, id).await, "discontinued");
}

// CGC-LC2: `discontinued` is terminal — no transition out (council domain-expert finding).
#[tokio::test]
async fn item_status_discontinued_is_terminal() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc
        .create_item_group(NewItemGroup {
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        })
        .await
        .unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let id = svc.create_item(item(&uq("SKU"), gid, uom)).await.expect("item");

    svc.transition_item_status(id, CatalogStatus::Discontinued).await.expect("active->discontinued");
    let err = svc.transition_item_status(id, CatalogStatus::Active).await.unwrap_err();
    assert!(
        matches!(err, CatalogWriteError::InvalidStatusTransition { .. }),
        "discontinued -> active must be rejected; got {err:?}"
    );
    // status unchanged — the rejected transition wrote nothing
    assert_eq!(status_of(&pool, id).await, "discontinued");
}

// C3: the RLS guard refuses a superuser connection (superusers bypass FORCE ROW LEVEL SECURITY).
// The dev/test DB connects as `postgres` (superuser), so the guard must reject here — proving it
// catches the exact failure mode it guards against (the half-fence posture, ADR-0029).
#[tokio::test]
async fn rls_guard_rejects_superuser_connection() {
    let pool = pool().await;
    let err = backbone_catalog::assert_rls_enforced(&pool).await.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("superuser"),
        "guard should name the superuser problem; got: {err}"
    );
}

// CGC-4: missing uom
#[tokio::test]
async fn item_rejects_missing_uom() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let err = svc.create_item(item(&uq("SKU"), gid, Uuid::new_v4())).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::UomNotFound(_)));
}

// CGC-5: no usage flag
#[tokio::test]
async fn item_rejects_no_usage_flag() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.is_sales_item = false; it.is_purchase_item = false; it.is_stock_item = false;
    let err = svc.create_item(it).await.unwrap_err();
    assert_eq!(err.code(), "no_usage_flag");
}

// CGC-6: item code uniqueness is NOT a module-level guarantee (ADR-0029). Every
// business unique in this module was company-leading, so the strip removed them all —
// the per-unit (org_unit_id, item_code) unique installs at composition, and the
// service still maps the resulting violation to `DuplicateItemCode` when a decorated
// host arms it. Undecorated (this test), a repeated code is just another row: the
// module must not grow a GLOBAL unique back, or it would fight the decorator's.
#[tokio::test]
async fn duplicate_item_code_is_the_decorators_unique() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let code = uq("SKU");
    let first = svc.create_item(item(&code, gid, uom)).await.expect("first");
    let second = svc.create_item(item(&code, gid, uom)).await.expect("second (no module-level unique)");
    assert_ne!(first, second, "two distinct rows with the same item_code, undecorated");
}

// ── Variant / attribute cases ──────────────────────────────────────────────

async fn seed_item(pool: &PgPool, svc: &CatalogWriteService) -> uuid::Uuid {
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(pool, &uq("PCS")).await;
    svc.create_item(item(&uq("SKU"), gid, uom)).await.unwrap()
}

async fn seed_attr_value(
    svc: &CatalogWriteService,
    attr_code: &str,
    val_code: &str,
    label: &str,
) {
    let aid = svc.create_attribute(NewAttribute {
        code: attr_code.into(), name: attr_code.into(), attribute_type: None,
    }).await.unwrap();
    svc.create_attribute_value(NewAttributeValue {
        attribute_id: aid, code: val_code.into(), label: label.into(),
        label_en: None, swatch_hex: None, sort_order: 0,
    }).await.unwrap();
}

// Attribute value requires an existing attribute.
#[tokio::test]
async fn attribute_value_rejects_missing_attribute() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let err = svc.create_attribute_value(NewAttributeValue {
        attribute_id: Uuid::new_v4(), code: "x".into(), label: "X".into(),
        label_en: None, swatch_hex: None, sort_order: 0,
    }).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::AttributeNotFound(_)));
}

// Happy: variant with valid options → label built from value labels, item.has_variants flips true.
#[tokio::test]
async fn item_variant_happy_sets_label_and_flag() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let item_id = seed_item(&pool, &svc).await;
    let color = uq("color"); let size = uq("size");
    seed_attr_value(&svc, &color, "red", "Red").await;
    seed_attr_value(&svc, &size, "m", "M").await;

    let mut opts = std::collections::BTreeMap::new();
    opts.insert(color.clone(), "red".to_string());
    opts.insert(size.clone(), "m".to_string());
    let vid = svc.create_item_variant(NewItemVariant {
        item_id, sku: uq("VAR"), variant_label: None, options: opts,
        barcode: None, is_default: true, weight_per_unit: None,
    }).await.expect("variant");

    let label: String = sqlx::query_scalar("SELECT variant_label FROM catalog.item_variants WHERE id=$1")
        .bind(vid).fetch_one(&pool).await.unwrap();
    // BTreeMap orders keys; label joins values in key order. Both "Red" and "M" present.
    assert!(label.contains("Red") && label.contains("M"), "label was {label}");

    let has: bool = sqlx::query_scalar("SELECT has_variants FROM catalog.items WHERE id=$1")
        .bind(item_id).fetch_one(&pool).await.unwrap();
    assert!(has, "item.has_variants must flip true");
}

// Unknown attribute value is rejected.
#[tokio::test]
async fn item_variant_rejects_unknown_value() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let item_id = seed_item(&pool, &svc).await;
    let color = uq("color");
    seed_attr_value(&svc, &color, "red", "Red").await;
    let mut opts = std::collections::BTreeMap::new();
    opts.insert(color, "purple".to_string()); // not a registered value
    let err = svc.create_item_variant(NewItemVariant {
        item_id, sku: uq("VAR"), variant_label: None, options: opts,
        barcode: None, is_default: false, weight_per_unit: None,
    }).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::UnknownAttributeValue(_)), "got {err:?}");
}

// Unknown attribute (axis) is rejected.
#[tokio::test]
async fn item_variant_rejects_unknown_attribute() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let item_id = seed_item(&pool, &svc).await;
    let mut opts = std::collections::BTreeMap::new();
    opts.insert(uq("ghost"), "x".to_string());
    let err = svc.create_item_variant(NewItemVariant {
        item_id, sku: uq("VAR"), variant_label: None, options: opts,
        barcode: None, is_default: false, weight_per_unit: None,
    }).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::UnknownAttribute(_)), "got {err:?}");
}

// Duplicate SKU is the decorator's unique (ADR-0029): the module ships no per-row
// uniqueness on `sku`, so undecorated a repeated SKU writes a second row. Under a
// composed host the decorator's (org_unit_id, sku) unique fires and the service maps
// the violation to `CatalogWriteError::DuplicateSku`.
#[tokio::test]
async fn duplicate_sku_is_the_decorators_unique() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let item_id = seed_item(&pool, &svc).await;
    let color = uq("color");
    seed_attr_value(&svc, &color, "red", "Red").await;
    let sku = uq("VAR");
    let mk = |c: &str| { let mut m = std::collections::BTreeMap::new(); m.insert(c.to_string(), "red".to_string()); m };
    let first = svc.create_item_variant(NewItemVariant {
        item_id, sku: sku.clone(), variant_label: None, options: mk(&color),
        barcode: None, is_default: false, weight_per_unit: None,
    }).await.expect("first");
    let second = svc.create_item_variant(NewItemVariant {
        item_id, sku, variant_label: None, options: mk(&color),
        barcode: None, is_default: false, weight_per_unit: None,
    }).await.expect("second (no module-level unique)");
    assert_ne!(first, second, "two distinct variant rows with the same sku, undecorated");
}

// ── Brand cases ────────────────────────────────────────────────────────────

// Item with a non-existent brand_id is rejected.
#[tokio::test]
async fn item_rejects_missing_brand() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.brand_id = Some(Uuid::new_v4()); // does not exist
    let err = svc.create_item(it).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::BrandNotFound(_)), "got {err:?}");
}

// Item with a valid brand persists the brand link.
#[tokio::test]
async fn item_with_brand_persists() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup {
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let brand = seed_brand(&pool, &uq("BOSCH")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.brand_id = Some(brand);
    let id = svc.create_item(it).await.expect("item");

    let bid: Option<Uuid> = sqlx::query_scalar("SELECT brand_id FROM catalog.items WHERE id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(bid, Some(brand));
}
