//! Barcode / SKU → item resolution (scan → item, step 1 of the counter journey). Requires
//! DATABASE_URL with the catalog schema (:5433).

use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::application::service::catalog_write_service::{
    CatalogWriteService, NewItem, NewItemGroup,
};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string());
    PgPool::connect(&url).await.unwrap()
}
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn seed_uom(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,$3)").bind(id).bind(code).bind(code).execute(pool).await.unwrap();
    id
}
fn new_item(code: &str, barcode: Option<String>, group: Uuid, uom: Uuid) -> NewItem {
    NewItem {
        item_code: code.into(), name: "Item".into(), description: None, barcode, brand_id: None,
        item_group_id: group, default_uom_id: uom, item_type: None, is_sales_item: true,
        is_purchase_item: true, is_stock_item: true, hsn_code: None, is_taxable: true,
        weight_per_unit: None, tags: None, data: None,
    }
}

#[tokio::test]
async fn scan_resolves_barcode_and_sku_to_an_item() {
    let pool = pool().await;
    let svc = CatalogWriteService::new(pool.clone());
    let gid = svc.create_item_group(NewItemGroup { code: uq("FG"), name: "G".into(), parent_id: None, is_group: false }).await.unwrap();
    let uom = seed_uom(&pool, &uq("PCS")).await;

    let sku = uq("SKU");
    let barcode = uq("BC");
    let item_id = svc.create_item(new_item(&sku, Some(barcode.clone()), gid, uom)).await.unwrap();

    // resolve by barcode.
    let hit = svc.lookup_item(&barcode).await.unwrap().expect("barcode resolves");
    assert_eq!(hit.item_id, item_id);
    assert_eq!(hit.variant_id, None);
    assert_eq!(hit.barcode.as_deref(), Some(barcode.as_str()));
    // resolve by item_code (SKU).
    let hit = svc.lookup_item(&sku).await.unwrap().expect("sku resolves");
    assert_eq!(hit.item_id, item_id);

    // a variant SKU/barcode resolves to the parent item + the variant. (Seeded directly — variant
    // creation validates option combinations, which is orthogonal to scan resolution.)
    let vsku = uq("VSKU");
    let vbc = uq("VBC");
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.item_variants (id, item_id, sku, variant_label, barcode, is_default) VALUES ($1,$2,$3,$4,$5,true)")
        .bind(variant_id).bind(item_id).bind(&vsku).bind("Red / M").bind(&vbc).execute(&pool).await.unwrap();
    let hit = svc.lookup_item(&vbc).await.unwrap().expect("variant barcode resolves");
    assert_eq!(hit.item_id, item_id);
    assert_eq!(hit.variant_id, Some(variant_id));
    assert_eq!(hit.sku.as_deref(), Some(vsku.as_str()));
    let hit = svc.lookup_item(&vsku).await.unwrap().expect("variant sku resolves");
    assert_eq!(hit.variant_id, Some(variant_id));

    // unknown code → no hit.
    assert!(svc.lookup_item(&uq("NOPE")).await.unwrap().is_none());
}
