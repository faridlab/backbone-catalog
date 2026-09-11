//! Council integrity probes — regression tests for the CRUD-bypass hole, at the ROUTE level.
//! The guarded composition must lock generic Item/ItemGroup/Uom writes and enforce
//! validation on the sanctioned create path (including the UoM tree link surface, ADR-0023).
//! Hits routes via tower oneshot (no live server).
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).
//!
//! Tenancy: none, by design (ADR-0029) — the module runs undecorated here: requests go
//! through with no org scope bound, and every seeded code carries a random suffix so
//! parallel cases on the shared database never collide.

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
/// Send a POST on the guarded surface.
async fn post(app: axum::Router, uri: &str, body: String) -> StatusCode {
    app.oneshot(
        Request::builder().method("POST").uri(uri)
            .header("content-type", "application/json").body(Body::from(body)).unwrap(),
    ).await.unwrap().status()
}
async fn send(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> StatusCode {
    let b = body.map(Body::from).unwrap_or(Body::empty());
    app.oneshot(
        Request::builder().method(method).uri(uri)
            .header("content-type", "application/json").body(b).unwrap(),
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

// IGC-3: validated tree link rejects a zero ratio (factors must stay positive).
#[tokio::test]
async fn guarded_tree_link_rejects_zero_ratio() {
    let pool = pool().await;
    let (_g, from) = seed_group_and_uom(&pool).await;
    let to = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'T')")
        .bind(to).bind(uq("TO")).execute(&pool).await.unwrap();
    let body = format!(r#"{{"relativeUomId":"{from}","relativeFactor":"0"}}"#);
    let status = post(create_guarded_catalog_routes(&module(&pool).await), &format!("/uoms/{to}/relative"), body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// IGC-4: valid item + valid tree child unit succeed via the guarded surface.
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

    // A child unit defined against the seeded root: 1 new unit = 12 of the root.
    let child_body = format!(
        r#"{{"code":"{}","name":"Dozen","relativeUomId":"{u}","relativeFactor":"12"}}"#,
        uq("DZN")
    );
    let s2 = post(create_guarded_catalog_routes(&module(&pool).await), "/uoms", child_body).await;
    assert_eq!(s2, StatusCode::CREATED);

    // The stored factor was derived on insert: root factor 1 x ratio 12.
    let factor: rust_decimal::Decimal = sqlx::query_scalar(
        "SELECT factor FROM catalog.uoms WHERE relative_uom_id = $1",
    )
    .bind(u)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(factor, rust_decimal::Decimal::from(12));
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

// IGC-8: validated Uom create via the guarded surface works. Code uniqueness is the
// composing decorator's (ADR-0029): the module ships no per-row unique on `code`
// (every business unique here was company-leading and the strip removed them), so
// undecorated a repeated code writes a second row; under a composed host the
// decorator's (org_unit_id, code) unique fires and the service maps the violation to
// `DuplicateUomCode` (422).
#[tokio::test]
async fn guarded_uom_create_repeats_code_undecorated() {
    let pool = pool().await;
    let code = uq("UOM");
    let body = format!(r#"{{"code":"{code}","name":"Pieces","uomType":"count"}}"#);
    let s1 = send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/uoms", Some(body.clone())).await;
    assert_eq!(s1, StatusCode::CREATED);
    let s2 = send(create_guarded_catalog_routes(&module(&pool).await), "POST", "/uoms", Some(body)).await;
    assert_eq!(s2, StatusCode::CREATED, "no module-level unique: the second row writes undecorated");
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
    send(app.clone(), "POST", "/attributes", Some(av)).await;
    let avv = format!(r#"{{"attributeId":"{}","code":"red","label":"Red"}}"#,
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM catalog.attributes WHERE code=$1").bind(&attr).fetch_one(&pool).await.unwrap());
    send(app.clone(), "POST", "/attribute-values", Some(avv)).await;
    let vbody = format!(r#"{{"itemId":"{item}","sku":"{}","options":{{"{attr}":"red"}}}}"#, uq("VAR"));
    let cs = send(app.clone(), "POST", "/item-variants", Some(vbody)).await;
    assert_eq!(cs, StatusCode::CREATED);

    let has1: bool = sqlx::query_scalar("SELECT has_variants FROM catalog.items WHERE id=$1").bind(item).fetch_one(&pool).await.unwrap();
    assert!(has1, "has_variants should be true after adding a variant");

    let vid: Uuid = sqlx::query_scalar("SELECT id FROM catalog.item_variants WHERE item_id=$1").bind(item).fetch_one(&pool).await.unwrap();
    let ds = send(app, "POST", "/item-variants/delete", Some(format!(r#"{{"id":"{vid}"}}"#))).await;
    assert_eq!(ds, StatusCode::OK);

    let has2: bool = sqlx::query_scalar("SELECT has_variants FROM catalog.items WHERE id=$1").bind(item).fetch_one(&pool).await.unwrap();
    assert!(!has2, "has_variants must flip back to false when the last variant is deleted");
}

// IGC-10: the validated Uom retire endpoint (UM-4) — a module-seeded (protected) unit
// refuses with 422, an unreferenced user unit archives with 200, and the unit a live
// item defaults to refuses with 422.
#[tokio::test]
async fn guarded_uom_retire_endpoint_enforces_protection() {
    let pool = pool().await;
    let (g, u) = seed_group_and_uom(&pool).await;
    let app = create_guarded_catalog_routes(&module(&pool).await);

    // A user unit nothing leans on retires cleanly.
    let lone = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'L')")
        .bind(lone).bind(uq("LONE")).execute(&pool).await.unwrap();
    let ok = send(app.clone(), "POST", "/uoms/delete", Some(format!(r#"{{"id":"{lone}"}}"#))).await;
    assert_eq!(ok, StatusCode::OK);

    // A protected (module-seeded) unit refuses.
    let seeded = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.uoms (id, code, name) VALUES ($1,$2,'S')")
        .bind(seeded).bind(uq("SEEDED")).execute(&pool).await.unwrap();
    sqlx::query("UPDATE catalog.uoms SET is_protected = true WHERE id = $1")
        .bind(seeded).execute(&pool).await.unwrap();
    let refused = send(app.clone(), "POST", "/uoms/delete", Some(format!(r#"{{"id":"{seeded}"}}"#))).await;
    assert_eq!(refused, StatusCode::UNPROCESSABLE_ENTITY);

    // A live item still pointing its default_uom at a unit makes that unit unretirable —
    // the orphaning hazard the endpoint exists for (ADR-005).
    let item = Uuid::new_v4();
    sqlx::query("INSERT INTO catalog.items (id, item_code, name, item_group_id, default_uom_id) VALUES ($1,$2,'T',$3,$4)")
        .bind(item).bind(uq("SKU2")).bind(g).bind(u).execute(&pool).await.unwrap();
    let in_use = send(app, "POST", "/uoms/delete", Some(format!(r#"{{"id":"{u}"}}"#))).await;
    assert_eq!(in_use, StatusCode::UNPROCESSABLE_ENTITY);
}
