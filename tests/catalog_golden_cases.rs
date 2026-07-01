//! Golden-case tests for the catalog validated write path.
//! Proves CatalogWriteService enforces the R1–R9 rules against real Postgres.
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::{
    CatalogWriteError, CatalogWriteService, NewItem, NewItemGroup, NewUomConversion,
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

async fn seed_uom(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,$3)")
        .bind(id).bind(code).bind(code)
        .execute(pool).await.unwrap();
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
        .create_item_group(NewItemGroup { code: uq("FG"), name: "Finished".into(), parent_id: None, is_group: false })
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

// PGC-1: a non-physical type (service/digital) is auto non-stockable, even if the caller asks.
#[tokio::test]
async fn non_physical_types_are_not_stockable() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("SVC"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
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
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
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

// CGC-4: missing uom
#[tokio::test]
async fn item_rejects_missing_uom() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let err = svc.create_item(item(&uq("SKU"), gid, Uuid::new_v4())).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::UomNotFound(_)));
}

// CGC-5: no usage flag
#[tokio::test]
async fn item_rejects_no_usage_flag() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.is_sales_item = false; it.is_purchase_item = false; it.is_stock_item = false;
    let err = svc.create_item(it).await.unwrap_err();
    assert_eq!(err.code(), "no_usage_flag");
}

// CGC-6: duplicate item code
#[tokio::test]
async fn item_rejects_duplicate_code() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let code = uq("SKU");
    svc.create_item(item(&code, gid, uom)).await.expect("first");
    let err = svc.create_item(item(&code, gid, uom)).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::DuplicateItemCode(_)));
}

// CGC-7: self conversion; CGC-8: non-positive; CGC-9: valid
#[tokio::test]
async fn uom_conversion_rules() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let boxu = seed_uom(&pool, &uq("BOX")).await;
    let pcs = seed_uom(&pool, &uq("PCS")).await;

    let same = svc.create_uom_conversion(NewUomConversion { from_uom_id: boxu, to_uom_id: boxu, factor: Decimal::from(2) }).await.unwrap_err();
    assert!(matches!(same, CatalogWriteError::SameUom));

    let zero = svc.create_uom_conversion(NewUomConversion { from_uom_id: boxu, to_uom_id: pcs, factor: Decimal::ZERO }).await.unwrap_err();
    assert!(matches!(zero, CatalogWriteError::NonPositiveFactor));

    let id = svc.create_uom_conversion(NewUomConversion { from_uom_id: boxu, to_uom_id: pcs, factor: Decimal::from(12) }).await.expect("valid");
    let f = sqlx::query_scalar::<_, Decimal>("SELECT factor FROM catalog.uom_conversions WHERE id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(f, Decimal::from(12));
}

// ── Variant / attribute cases ──────────────────────────────────────────────
use backbone_catalog::{NewAttribute, NewAttributeValue, NewItemVariant};

async fn seed_item(pool: &PgPool, svc: &CatalogWriteService) -> uuid::Uuid {
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let uom = seed_uom(pool, &uq("PCS")).await;
    svc.create_item(item(&uq("SKU"), gid, uom)).await.unwrap()
}

async fn seed_attr_value(svc: &CatalogWriteService, attr_code: &str, val_code: &str, label: &str) {
    let aid = svc.create_attribute(NewAttribute { code: attr_code.into(), name: attr_code.into(), attribute_type: None }).await.unwrap();
    svc.create_attribute_value(NewAttributeValue { attribute_id: aid, code: val_code.into(), label: label.into(), label_en: None, swatch_hex: None, sort_order: 0 }).await.unwrap();
}

// Attribute value requires an existing attribute.
#[tokio::test]
async fn attribute_value_rejects_missing_attribute() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let err = svc.create_attribute_value(NewAttributeValue { attribute_id: Uuid::new_v4(), code: "x".into(), label: "X".into(), label_en: None, swatch_hex: None, sort_order: 0 }).await.unwrap_err();
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
        item_id, sku: uq("VAR"), variant_label: None, options: opts, barcode: None, is_default: true, weight_per_unit: None,
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
    let err = svc.create_item_variant(NewItemVariant { item_id, sku: uq("VAR"), variant_label: None, options: opts, barcode: None, is_default: false, weight_per_unit: None }).await.unwrap_err();
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
    let err = svc.create_item_variant(NewItemVariant { item_id, sku: uq("VAR"), variant_label: None, options: opts, barcode: None, is_default: false, weight_per_unit: None }).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::UnknownAttribute(_)), "got {err:?}");
}

// Duplicate SKU is rejected.
#[tokio::test]
async fn item_variant_rejects_duplicate_sku() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let item_id = seed_item(&pool, &svc).await;
    let color = uq("color");
    seed_attr_value(&svc, &color, "red", "Red").await;
    let sku = uq("VAR");
    let mk = |c: &str| { let mut m = std::collections::BTreeMap::new(); m.insert(c.to_string(), "red".to_string()); m };
    svc.create_item_variant(NewItemVariant { item_id, sku: sku.clone(), variant_label: None, options: mk(&color), barcode: None, is_default: false, weight_per_unit: None }).await.expect("first");
    let err = svc.create_item_variant(NewItemVariant { item_id, sku, variant_label: None, options: mk(&color), barcode: None, is_default: false, weight_per_unit: None }).await.unwrap_err();
    assert!(matches!(err, CatalogWriteError::DuplicateSku(_)), "got {err:?}");
}

// ── Brand cases ────────────────────────────────────────────────────────────
async fn seed_brand(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.brands (id, code, name) VALUES ($1,$2,$3)")
        .bind(id).bind(code).bind(code).execute(pool).await.unwrap();
    id
}

// Item with a non-existent brand_id is rejected.
#[tokio::test]
async fn item_rejects_missing_brand() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
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
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;
    let brand = seed_brand(&pool, &uq("BOSCH")).await;
    let mut it = item(&uq("SKU"), gid, uom);
    it.brand_id = Some(brand);
    let id = svc.create_item(it).await.expect("item");

    let bid: Option<Uuid> = sqlx::query_scalar("SELECT brand_id FROM catalog.items WHERE id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(bid, Some(brand));
}
