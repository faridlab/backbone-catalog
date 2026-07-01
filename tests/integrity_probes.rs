//! Council integrity probes — regression tests for the CRUD-bypass hole, at the ROUTE level.
//! The guarded composition must lock generic Item/ItemGroup/UomConversion writes and enforce
//! validation on the sanctioned create path. Hits routes via tower oneshot (no live server).
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use backbone_catalog::{create_guarded_catalog_routes, CatalogModule};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_catalog".to_string()
    });
    PgPool::connect(&url).await.unwrap()
}
async fn module(pool: &PgPool) -> CatalogModule {
    CatalogModule::builder().with_database(pool.clone()).build().unwrap()
}
async fn post(app: axum::Router, uri: &str, body: String) -> StatusCode {
    app.oneshot(
        Request::builder().method("POST").uri(uri)
            .header("content-type", "application/json").body(Body::from(body)).unwrap(),
    ).await.unwrap().status()
}
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }

async fn seed_group_and_uom(pool: &PgPool) -> (Uuid, Uuid) {
    let g = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.item_groups (id, code, name) VALUES ($1,$2,'G')")
        .bind(g).bind(uq("FG")).execute(pool).await.unwrap();
    let u = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'PCS')")
        .bind(u).bind(uq("PCS")).execute(pool).await.unwrap();
    (g, u)
}

// IGC-1: generic item create is not mounted on the guarded surface.
#[tokio::test]
async fn guarded_routes_lock_generic_item_create() {
    let pool = pool().await;
    let (g, u) = seed_group_and_uom(&pool).await;
    // A fully-formed generic CreateItemDto payload (camelCase) — would 201 on raw routes().
    let body = format!(
        r#"{{"itemCode":"{}","name":"X","itemGroupId":"{g}","defaultUomId":"{u}","itemType":"physical_good","isSalesItem":true,"isPurchaseItem":true,"isStockItem":true,"isTaxable":true,"status":"active"}}"#,
        uq("BYP")
    );
    // Hit the generic verb by targeting a route only raw CRUD would add for items via PATCH/upsert;
    // on the guarded surface POST /items is the VALIDATED handler, so instead prove the raw CRUD
    // bulk endpoint is absent.
    let status = post(create_guarded_catalog_routes(&module(&pool).await), "/items/bulk", body).await;
    assert!(
        status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_FOUND,
        "generic bulk item create must not be exposed; got {status}"
    );
}

// IGC-2: validated item create rejects a missing item group.
#[tokio::test]
async fn guarded_item_rejects_missing_group() {
    let pool = pool().await;
    let (_g, u) = seed_group_and_uom(&pool).await;
    let body = format!(
        r#"{{"itemCode":"{}","name":"X","itemGroupId":"{}","defaultUomId":"{u}"}}"#,
        uq("SKU"), Uuid::new_v4()
    );
    let status = post(create_guarded_catalog_routes(&module(&pool).await), "/items", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// IGC-3: validated conversion rejects factor = 0.
#[tokio::test]
async fn guarded_conversion_rejects_zero_factor() {
    let pool = pool().await;
    let (_g, from) = seed_group_and_uom(&pool).await;
    let to = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'T')")
        .bind(to).bind(uq("TO")).execute(&pool).await.unwrap();
    let body = format!(r#"{{"fromUomId":"{from}","toUomId":"{to}","factor":"0"}}"#);
    let status = post(create_guarded_catalog_routes(&module(&pool).await), "/uom-conversions", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// IGC-4: valid item + valid conversion succeed via the guarded surface.
#[tokio::test]
async fn guarded_valid_writes_succeed() {
    let pool = pool().await;
    let (g, u) = seed_group_and_uom(&pool).await;
    let item_body = format!(
        r#"{{"itemCode":"{}","name":"Ok","itemGroupId":"{g}","defaultUomId":"{u}"}}"#,
        uq("OK")
    );
    let s1 = post(create_guarded_catalog_routes(&module(&pool).await), "/items", item_body).await;
    assert_eq!(s1, StatusCode::CREATED);

    let to = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'T')")
        .bind(to).bind(uq("TO")).execute(&pool).await.unwrap();
    let conv_body = format!(r#"{{"fromUomId":"{u}","toUomId":"{to}","factor":"12"}}"#);
    let s2 = post(create_guarded_catalog_routes(&module(&pool).await), "/uom-conversions", conv_body).await;
    assert_eq!(s2, StatusCode::CREATED);
}

// IGC-5: validated item-variant create rejects an unknown attribute value (route-level).
#[tokio::test]
async fn guarded_item_variant_rejects_unknown_option() {
    let pool = pool().await;
    let app = create_guarded_catalog_routes(&module(&pool).await);
    // Seed a template item directly.
    let (g, u) = seed_group_and_uom(&pool).await;
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.items (id, item_code, name, item_group_id, default_uom_id) VALUES ($1,$2,'T',$3,$4)")
        .bind(item).bind(uq("SKU")).bind(g).bind(u).execute(&pool).await.unwrap();
    // Options reference an attribute axis that doesn't exist.
    let body = format!(r#"{{"itemId":"{item}","sku":"{}","options":{{"ghost":"x"}}}}"#, uq("VAR"));
    let status = post(app, "/item-variants", body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ── Council 2026-07-01: Uom/Brand orphan-parent hole + has_variants latch ──────────

async fn send(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> StatusCode {
    let b = body.map(Body::from).unwrap_or(Body::empty());
    app.oneshot(
        Request::builder().method(method).uri(uri)
            .header("content-type", "application/json").body(b).unwrap(),
    ).await.unwrap().status()
}

// IGC-6: the guarded surface does NOT expose generic delete/patch on Uom (would orphan items).
#[tokio::test]
async fn guarded_routes_lock_uom_mutation() {
    let pool = pool().await;
    let (_g, u) = seed_group_and_uom(&pool).await;
    for (method, uri) in [("DELETE", format!("/uoms/{u}")), ("PATCH", format!("/uoms/{u}"))] {
        let status = send(create_guarded_catalog_routes(&module(&pool).await), method, &uri, Some("{}".into())).await;
        assert!(
            status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_FOUND,
            "{method} {uri} must not be exposed on the guarded surface; got {status}"
        );
    }
}

// IGC-7: same lock for Brand.
#[tokio::test]
async fn guarded_routes_lock_brand_mutation() {
    let pool = pool().await;
    let bid = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.brands (id, code, name) VALUES ($1,$2,'B')")
        .bind(bid).bind(uq("BR")).execute(&pool).await.unwrap();
    for (method, uri) in [("DELETE", format!("/brands/{bid}")), ("PATCH", format!("/brands/{bid}"))] {
        let status = send(create_guarded_catalog_routes(&module(&pool).await), method, &uri, Some("{}".into())).await;
        assert!(
            status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_FOUND,
            "{method} {uri} must not be exposed; got {status}"
        );
    }
}

// IGC-8: validated Uom create via the guarded surface works and dedupes.
#[tokio::test]
async fn guarded_uom_create_and_dedupe() {
    let pool = pool().await;
    let code = uq("UOM");
    let body = format!(r#"{{"code":"{code}","name":"Pieces","uomType":"count"}}"#);
    let s1 = send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/uoms", Some(body.clone())).await;
    assert_eq!(s1, StatusCode::CREATED);
    let s2 = send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/uoms", Some(body)).await;
    assert_eq!(s2, StatusCode::UNPROCESSABLE_ENTITY, "duplicate uom code must be rejected");
}

// IGC-9: deleting the last variant flips the item's has_variants back to false (no lying flag).
#[tokio::test]
async fn deleting_last_variant_resets_has_variants() {
    let pool = pool().await;
    let (g, u) = seed_group_and_uom(&pool).await;
    let app = create_guarded_catalog_routes(&module(&pool).await);
    // template item
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.items (id, item_code, name, item_group_id, default_uom_id) VALUES ($1,$2,'T',$3,$4)")
        .bind(item).bind(uq("SKU")).bind(g).bind(u).execute(&pool).await.unwrap();
    // attribute + value, then a variant via guarded routes
    let attr = uq("color");
    let av = format!(r#"{{"code":"{attr}","name":"Color"}}"#);
    send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/attributes", Some(av)).await;
    let avv = format!(r#"{{"attributeId":"{}","code":"red","label":"Red"}}"#,
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM catalog.attributes WHERE code=$1").bind(&attr).fetch_one(&pool).await.unwrap());
    send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/attribute-values", Some(avv)).await;
    let vbody = format!(r#"{{"itemId":"{item}","sku":"{}","options":{{"{attr}":"red"}}}}"#, uq("VAR"));
    let cs = send(app, "POST", "/item-variants", Some(vbody)).await;
    assert_eq!(cs, StatusCode::CREATED);

    let has1: bool = sqlx::query_scalar("SELECT has_variants FROM catalog.items WHERE id=$1").bind(item).fetch_one(&pool).await.unwrap();
    assert!(has1, "has_variants should be true after adding a variant");

    let vid: Uuid = sqlx::query_scalar("SELECT id FROM catalog.item_variants WHERE item_id=$1").bind(item).fetch_one(&pool).await.unwrap();
    let ds = send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/item-variants/delete", Some(format!(r#"{{"id":"{vid}"}}"#))).await;
    assert_eq!(ds, StatusCode::OK);

    let has2: bool = sqlx::query_scalar("SELECT has_variants FROM catalog.items WHERE id=$1").bind(item).fetch_one(&pool).await.unwrap();
    assert!(!has2, "has_variants must flip back to false when the last variant is deleted");
}
