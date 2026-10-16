//! An item and the deletion of its unit cannot cross: a create waits for a delete already under
//! way and then refuses, and a delete waits for a create already under way. Each test holds one
//! side open in a raw transaction and drives the other through the service. Requires DATABASE_URL.

use std::time::Duration;

use sqlx::PgPool;
use uuid::Uuid;

use backbone_catalog::{CatalogWriteError, CatalogWriteService, NewItem, NewItemGroup, NewUom};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string());
    PgPool::connect(&url).await.unwrap()
}

fn uq(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn unit(svc: &CatalogWriteService) -> Uuid {
    svc.create_uom(NewUom {
        code: uq("U"), name: "Unit".into(), uom_type: None, decimal_places: 0,
        relative_uom_id: None, relative_factor: None,
    })
    .await
    .unwrap()
}

fn item(group: Uuid, uom: Uuid) -> NewItem {
    NewItem {
        item_code: uq("ITEM"), name: "Item".into(), description: None, barcode: None, brand_id: None,
        item_group_id: group, default_uom_id: uom, item_type: None, is_sales_item: true,
        is_purchase_item: false, is_stock_item: false, hsn_code: None, is_taxable: true,
        weight_per_unit: None, standard_cost: None, tags: None, data: None,
    }
}

#[tokio::test]
async fn a_create_waits_for_a_delete_of_its_unit_and_refuses() {
    let pool = pool().await;
    let svc = std::sync::Arc::new(CatalogWriteService::new(pool.clone()));
    let uom = unit(&svc).await;
    let group = svc
        .create_item_group(NewItemGroup { code: uq("GRP"), name: "Group".into(), parent_id: None, is_group: false })
        .await
        .unwrap();

    // A delete under way: the unit locked and retired, not yet committed.
    let mut deleting = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM catalog.uoms WHERE id = $1 FOR UPDATE").bind(uom).execute(&mut *deleting).await.unwrap();
    sqlx::query("UPDATE catalog.uoms SET metadata = metadata || jsonb_build_object('deleted_at', now()) WHERE id = $1")
        .bind(uom).execute(&mut *deleting).await.unwrap();

    let create = tokio::spawn({
        let svc = svc.clone();
        async move { svc.create_item(item(group, uom)).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!create.is_finished(), "the create waits for the delete under way");
    deleting.commit().await.unwrap();
    let created = create.await.unwrap();
    assert!(matches!(created, Err(CatalogWriteError::UomNotFound(id)) if id == uom), "{created:?}");
}

#[tokio::test]
async fn a_delete_waits_for_a_create_on_its_unit_and_then_finds_the_item() {
    let pool = pool().await;
    let svc = std::sync::Arc::new(CatalogWriteService::new(pool.clone()));
    let uom = unit(&svc).await;
    let group = svc
        .create_item_group(NewItemGroup { code: uq("GRP"), name: "Group".into(), parent_id: None, is_group: false })
        .await
        .unwrap();

    // A create under way: the unit share-locked as create_item holds it, the item written, not
    // yet committed.
    let mut creating = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM catalog.uoms WHERE id = $1 FOR SHARE").bind(uom).execute(&mut *creating).await.unwrap();
    sqlx::query("INSERT INTO catalog.items (id, item_code, name, item_group_id, default_uom_id) VALUES ($1, $2, 'Item', $3, $4)")
        .bind(Uuid::new_v4()).bind(uq("ITEM")).bind(group).bind(uom)
        .execute(&mut *creating).await.unwrap();

    let delete = tokio::spawn({
        let svc = svc.clone();
        async move { svc.delete_uom(uom).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!delete.is_finished(), "the delete waits for the create under way");
    creating.commit().await.unwrap();
    let deleted = delete.await.unwrap();
    assert!(matches!(deleted, Err(CatalogWriteError::UomInUse { .. })), "the unit now carries an item: {deleted:?}");
}
