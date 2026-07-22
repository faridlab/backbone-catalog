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
async fn seed_uom(pool: &PgPool, company: Uuid, code: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, company_id, code, name) VALUES ($1,$2,$3,$4)")
        .bind(id).bind(company).bind(code).bind(code).execute(pool).await.unwrap();
    id
}
fn new_item(company: Uuid, code: &str, barcode: Option<String>, group: Uuid, uom: Uuid) -> NewItem {
    NewItem {
        company_id: company,
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
    // ADR-0010 B1: catalog is tenant-scoped. The test uses a sentinel company_id and binds it
    // into the request scope so lookup_item's company filter sees the rows it just seeded.
    let company = Uuid::new_v4();
    let gid = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.create_item_group(NewItemGroup {
            company_id: company,
            code: uq("FG"), name: "G".into(), parent_id: None, is_group: false,
        }).await.unwrap()
    }).await;
    let uom = seed_uom(&pool, company, &uq("PCS")).await;

    let sku = uq("SKU");
    let barcode = uq("BC");
    let item_id = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.create_item(new_item(company, &sku, Some(barcode.clone()), gid, uom)).await.unwrap()
    }).await;

    // resolve by barcode. lookup_item reads the company from the task-local scope.
    let hit = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.lookup_item(&barcode).await.unwrap().expect("barcode resolves")
    }).await;
    assert_eq!(hit.item_id, item_id);
    assert_eq!(hit.variant_id, None);
    assert_eq!(hit.barcode.as_deref(), Some(barcode.as_str()));
    // resolve by item_code (SKU).
    let hit = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.lookup_item(&sku).await.unwrap().expect("sku resolves")
    }).await;
    assert_eq!(hit.item_id, item_id);

    // a variant SKU/barcode resolves to the parent item + the variant. (Seeded directly — variant
    // creation validates option combinations, which is orthogonal to scan resolution.)
    let vsku = uq("VSKU");
    let vbc = uq("VBC");
    let variant_id = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.item_variants (id, company_id, item_id, sku, variant_label, barcode, is_default) VALUES ($1,$2,$3,$4,$5,$6,true)")
        .bind(variant_id).bind(company).bind(item_id).bind(&vsku).bind("Red / M").bind(&vbc).execute(&pool).await.unwrap();
    let hit = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.lookup_item(&vbc).await.unwrap().expect("variant barcode resolves")
    }).await;
    assert_eq!(hit.item_id, item_id);
    assert_eq!(hit.variant_id, Some(variant_id));
    assert_eq!(hit.sku.as_deref(), Some(vsku.as_str()));
    let hit = backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.lookup_item(&vsku).await.unwrap().expect("variant sku resolves")
    }).await;
    assert_eq!(hit.variant_id, Some(variant_id));

    // unknown code → no hit.
    assert!(backbone_orm::company_scope::with_company_scope(Some(company), async {
        svc.lookup_item(&uq("NOPE")).await.unwrap().is_none()
    }).await);
}
