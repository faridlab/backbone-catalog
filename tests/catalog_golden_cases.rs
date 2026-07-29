//! Golden-case tests for the catalog validated write path.
//! Proves CatalogWriteService enforces the R1–R9 rules against real Postgres.
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).
//!
//! ADR-0010 B1: every test runs inside a fresh `with_company_scope(Some(company), …)` so the
//! validated write service's task-local `current_company()` is set, and every New* struct carries
//! the same `company_id`. A fresh random company per test keeps cases isolated even when the
//! tests share a database.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::{
    CatalogStatus, CatalogWriteError, CatalogWriteService, NewAttribute, NewAttributeValue, NewItem,
    NewItemGroup, NewItemVariant, NewUomConversion,
};
use backbone_orm::company_scope;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string()
    });
    PgPool::connect(&url).await.unwrap()
}

fn uq(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn seed_uom(pool: &PgPool, company: Uuid, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, company_id, code, name) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(company)
        .bind(code)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_brand(pool: &PgPool, company: Uuid, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.brands (id, company_id, code, name) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(company)
        .bind(code)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

fn item(company: Uuid, code: &str, group: Uuid, uom: Uuid) -> NewItem {
    NewItem {
        company_id: company,
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
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc
            .create_item_group(NewItemGroup {
                company_id: company,
                code: uq("FG"), name: "Finished".into(), parent_id: None, is_group: false,
            })
            .await
            .expect("group");
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let id = svc.create_item(item(company, &uq("SKU"), gid, uom)).await.expect("item");

        let row = sqlx::query_scalar::<_, String>(
            "SELECT item_type::text FROM catalog.items WHERE id=$1",
        )
        .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(row, "physical_good");
    }).await;
}

// PGC-1: a non-physical type (service/digital) is auto non-stockable, even if the caller asks.
#[tokio::test]
async fn non_physical_types_are_not_stockable() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("SVC"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("UNIT")).await;
        for kind in ["service", "digital_good", "subscription", "gift_card"] {
            let mut it = item(company, &uq("SKU"), gid, uom);
            it.item_type = Some(kind.to_string());
            it.is_stock_item = true; // caller asks for stock…
            it.is_sales_item = true;
            it.data = Some(serde_json::json!({"note": kind}));
            let id = svc.create_item(it).await.unwrap_or_else(|e| panic!("{kind}: {e:?}"));
            let stock: bool = sqlx::query_scalar("SELECT is_stock_item FROM catalog.items WHERE id=$1")
                .bind(id).fetch_one(&pool).await.unwrap();
            assert!(!stock, "{kind} must be non-stockable");
        }
    }).await;
}

// PGC-2: a physical good keeps stockability + persists tags/data.
#[tokio::test]
async fn physical_good_keeps_stock_and_tags() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let mut it = item(company, &uq("SKU"), gid, uom);
        it.item_type = Some("physical_good".into());
        it.is_stock_item = true;
        it.tags = Some(serde_json::json!(["bor", "listrik"]));
        let id = svc.create_item(it).await.expect("item");
        let (stock, tags): (bool, serde_json::Value) =
            sqlx::query_as("SELECT is_stock_item, tags FROM catalog.items WHERE id=$1")
                .bind(id).fetch_one(&pool).await.unwrap();
        assert!(stock);
        assert_eq!(tags, serde_json::json!(["bor", "listrik"]));
    }).await;
}

// CGC-3: missing item group
#[tokio::test]
async fn item_rejects_missing_group() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let err = svc.create_item(item(company, &uq("SKU"), Uuid::new_v4(), uom)).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::ItemGroupNotFound(_)));
    }).await;
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
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc
            .create_item_group(NewItemGroup {
                company_id: company, code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
            })
            .await
            .unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let id = svc.create_item(item(company, &uq("SKU"), gid, uom)).await.expect("item");

        svc.transition_item_status(id, CatalogStatus::Inactive).await.expect("active->inactive");
        assert_eq!(status_of(&pool, id).await, "inactive");
        svc.transition_item_status(id, CatalogStatus::Active).await.expect("inactive->active");
        assert_eq!(status_of(&pool, id).await, "active");
        svc.transition_item_status(id, CatalogStatus::Discontinued).await.expect("active->discontinued");
        assert_eq!(status_of(&pool, id).await, "discontinued");
    })
    .await;
}

// CGC-LC2: `discontinued` is terminal — no transition out (council domain-expert finding).
#[tokio::test]
async fn item_status_discontinued_is_terminal() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc
            .create_item_group(NewItemGroup {
                company_id: company, code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
            })
            .await
            .unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let id = svc.create_item(item(company, &uq("SKU"), gid, uom)).await.expect("item");

        svc.transition_item_status(id, CatalogStatus::Discontinued).await.expect("active->discontinued");
        let err = svc.transition_item_status(id, CatalogStatus::Active).await.unwrap_err();
        assert!(
            matches!(err, CatalogWriteError::InvalidStatusTransition { .. }),
            "discontinued -> active must be rejected; got {err:?}"
        );
        // status unchanged — the rejected transition wrote nothing
        assert_eq!(status_of(&pool, id).await, "discontinued");
    })
    .await;
}

// CGC-UOM1: conversion_factor resolves the direct row AND the inverse of the reverse row
// (UomConversion is stored one-directional; the lookup makes it usable both ways).
#[tokio::test]
async fn uom_conversion_factor_is_bidirectional() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let box_id = seed_uom(&pool, company, &uq("BOX")).await;
        let pcs_id = seed_uom(&pool, company, &uq("PCS")).await;
        // Store one direction only: 1 BOX = 12 PCS.
        svc.create_uom_conversion(NewUomConversion {
            company_id: company, from_uom_id: box_id, to_uom_id: pcs_id, factor: Decimal::from(12),
        })
        .await
        .expect("create box->pcs");

        // Direct: BOX -> PCS = 12.
        assert_eq!(svc.conversion_factor(box_id, pcs_id).await.unwrap(), Some(Decimal::from(12)));
        // Inverse: PCS -> BOX = 1/12.
        let inv = svc.conversion_factor(pcs_id, box_id).await.unwrap().expect("inverse present");
        assert_eq!(inv, Decimal::ONE / Decimal::from(12));
        // Unlinked unit -> None.
        let other = seed_uom(&pool, company, &uq("KG")).await;
        assert_eq!(svc.conversion_factor(box_id, other).await.unwrap(), None);
    })
    .await;
}

// CGC-UOM2: a redundant reverse row is rejected — one canonical factor per pair.
#[tokio::test]
async fn uom_conversion_rejects_redundant_reverse() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let a = seed_uom(&pool, company, &uq("BOX")).await;
        let b = seed_uom(&pool, company, &uq("PCS")).await;
        svc.create_uom_conversion(NewUomConversion {
            company_id: company, from_uom_id: a, to_uom_id: b, factor: Decimal::from(12),
        })
        .await
        .expect("create a->b");

        // Reverse (b->a) must be rejected: the pair is already convertible via conversion_factor.
        let err = svc
            .create_uom_conversion(NewUomConversion {
                company_id: company,
                from_uom_id: b,
                to_uom_id: a,
                factor: Decimal::ONE / Decimal::from(12),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, CatalogWriteError::DuplicateConversion),
            "redundant reverse must be rejected; got {err:?}"
        );
    })
    .await;
}

// C3: the RLS guard refuses a superuser connection (superusers bypass FORCE ROW LEVEL SECURITY).
// The dev/test DB connects as `postgres` (superuser), so the guard must reject here — proving it
// catches the exact failure mode the council's C3 finding is about.
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
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let err = svc.create_item(item(company, &uq("SKU"), gid, Uuid::new_v4())).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::UomNotFound(_)));
    }).await;
}

// CGC-5: no usage flag
#[tokio::test]
async fn item_rejects_no_usage_flag() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let mut it = item(company, &uq("SKU"), gid, uom);
        it.is_sales_item = false; it.is_purchase_item = false; it.is_stock_item = false;
        let err = svc.create_item(it).await.unwrap_err();
        assert_eq!(err.code(), "no_usage_flag");
    }).await;
}

// CGC-6: duplicate item code
#[tokio::test]
async fn item_rejects_duplicate_code() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let code = uq("SKU");
        svc.create_item(item(company, &code, gid, uom)).await.expect("first");
        let err = svc.create_item(item(company, &code, gid, uom)).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::DuplicateItemCode(_)));
    }).await;
}

// CGC-7: self conversion; CGC-8: non-positive; CGC-9: valid
#[tokio::test]
async fn uom_conversion_rules() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let boxu = seed_uom(&pool, company, &uq("BOX")).await;
        let pcs = seed_uom(&pool, company, &uq("PCS")).await;

        let same = svc.create_uom_conversion(NewUomConversion {
            company_id: company, from_uom_id: boxu, to_uom_id: boxu, factor: Decimal::from(2),
        }).await.unwrap_err();
        assert!(matches!(same, CatalogWriteError::SameUom));

        let zero = svc.create_uom_conversion(NewUomConversion {
            company_id: company, from_uom_id: boxu, to_uom_id: pcs, factor: Decimal::ZERO,
        }).await.unwrap_err();
        assert!(matches!(zero, CatalogWriteError::NonPositiveFactor));

        let id = svc.create_uom_conversion(NewUomConversion {
            company_id: company, from_uom_id: boxu, to_uom_id: pcs, factor: Decimal::from(12),
        }).await.expect("valid");
        let f = sqlx::query_scalar::<_, Decimal>("SELECT factor FROM catalog.uom_conversions WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(f, Decimal::from(12));
    }).await;
}

// ── Variant / attribute cases ──────────────────────────────────────────────

async fn seed_item(pool: &PgPool, svc: &CatalogWriteService, company: Uuid) -> uuid::Uuid {
    let gid = svc.create_item_group(NewItemGroup {
        company_id: company,
        code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
    }).await.unwrap();
    let uom = seed_uom(pool, company, &uq("PCS")).await;
    svc.create_item(item(company, &uq("SKU"), gid, uom)).await.unwrap()
}

async fn seed_attr_value(
    svc: &CatalogWriteService,
    company: Uuid,
    attr_code: &str,
    val_code: &str,
    label: &str,
) {
    let aid = svc.create_attribute(NewAttribute {
        company_id: company,
        code: attr_code.into(), name: attr_code.into(), attribute_type: None,
    }).await.unwrap();
    svc.create_attribute_value(NewAttributeValue {
        company_id: company,
        attribute_id: aid, code: val_code.into(), label: label.into(),
        label_en: None, swatch_hex: None, sort_order: 0,
    }).await.unwrap();
}

// Attribute value requires an existing attribute.
#[tokio::test]
async fn attribute_value_rejects_missing_attribute() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let err = svc.create_attribute_value(NewAttributeValue {
            company_id: company,
            attribute_id: Uuid::new_v4(), code: "x".into(), label: "X".into(),
            label_en: None, swatch_hex: None, sort_order: 0,
        }).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::AttributeNotFound(_)));
    }).await;
}

// Happy: variant with valid options → label built from value labels, item.has_variants flips true.
#[tokio::test]
async fn item_variant_happy_sets_label_and_flag() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let item_id = seed_item(&pool, &svc, company).await;
        let color = uq("color"); let size = uq("size");
        seed_attr_value(&svc, company, &color, "red", "Red").await;
        seed_attr_value(&svc, company, &size, "m", "M").await;

        let mut opts = std::collections::BTreeMap::new();
        opts.insert(color.clone(), "red".to_string());
        opts.insert(size.clone(), "m".to_string());
        let vid = svc.create_item_variant(NewItemVariant {
            company_id: company,
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
    }).await;
}

// Unknown attribute value is rejected.
#[tokio::test]
async fn item_variant_rejects_unknown_value() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let item_id = seed_item(&pool, &svc, company).await;
        let color = uq("color");
        seed_attr_value(&svc, company, &color, "red", "Red").await;
        let mut opts = std::collections::BTreeMap::new();
        opts.insert(color, "purple".to_string()); // not a registered value
        let err = svc.create_item_variant(NewItemVariant {
            company_id: company,
            item_id, sku: uq("VAR"), variant_label: None, options: opts,
            barcode: None, is_default: false, weight_per_unit: None,
        }).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::UnknownAttributeValue(_)), "got {err:?}");
    }).await;
}

// Unknown attribute (axis) is rejected.
#[tokio::test]
async fn item_variant_rejects_unknown_attribute() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let item_id = seed_item(&pool, &svc, company).await;
        let mut opts = std::collections::BTreeMap::new();
        opts.insert(uq("ghost"), "x".to_string());
        let err = svc.create_item_variant(NewItemVariant {
            company_id: company,
            item_id, sku: uq("VAR"), variant_label: None, options: opts,
            barcode: None, is_default: false, weight_per_unit: None,
        }).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::UnknownAttribute(_)), "got {err:?}");
    }).await;
}

// Duplicate SKU is rejected.
#[tokio::test]
async fn item_variant_rejects_duplicate_sku() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let item_id = seed_item(&pool, &svc, company).await;
        let color = uq("color");
        seed_attr_value(&svc, company, &color, "red", "Red").await;
        let sku = uq("VAR");
        let mk = |c: &str| { let mut m = std::collections::BTreeMap::new(); m.insert(c.to_string(), "red".to_string()); m };
        svc.create_item_variant(NewItemVariant {
            company_id: company,
            item_id, sku: sku.clone(), variant_label: None, options: mk(&color),
            barcode: None, is_default: false, weight_per_unit: None,
        }).await.expect("first");
        let err = svc.create_item_variant(NewItemVariant {
            company_id: company,
            item_id, sku, variant_label: None, options: mk(&color),
            barcode: None, is_default: false, weight_per_unit: None,
        }).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::DuplicateSku(_)), "got {err:?}");
    }).await;
}

// ── Brand cases ────────────────────────────────────────────────────────────

// Item with a non-existent brand_id is rejected.
#[tokio::test]
async fn item_rejects_missing_brand() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let mut it = item(company, &uq("SKU"), gid, uom);
        it.brand_id = Some(Uuid::new_v4()); // does not exist
        let err = svc.create_item(it).await.unwrap_err();
        assert!(matches!(err, CatalogWriteError::BrandNotFound(_)), "got {err:?}");
    }).await;
}

// Item with a valid brand persists the brand link.
#[tokio::test]
async fn item_with_brand_persists() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    company_scope::with_company_scope(Some(company), async {
        let gid = svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap();
        let uom = seed_uom(&pool, company, &uq("PCS")).await;
        let brand = seed_brand(&pool, company, &uq("BOSCH")).await;
        let mut it = item(company, &uq("SKU"), gid, uom);
        it.brand_id = Some(brand);
        let id = svc.create_item(it).await.expect("item");

        let bid: Option<Uuid> = sqlx::query_scalar("SELECT brand_id FROM catalog.items WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(bid, Some(brand));
    }).await;
}
