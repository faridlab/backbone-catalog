//! Guarded route composition — the RECOMMENDED way to mount the catalog module.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Closes the CRUD-bypass: the generated
//! `routes()` exposes full mutable CRUD on every entity, backed by generic services with NO domain
//! validation. That lets a caller create an Item pointing at a missing item-group/UOM, an Item with
//! no usage flag, or a self-referential/non-positive UOM conversion — corrupting the product
//! identity every downstream module projects.
//!
//! Guarded surface:
//!   - **Item / ItemGroup / UomConversion**: READ + **validated create** via `CatalogWriteService`.
//!     Generic update/delete/upsert/bulk are intentionally NOT mounted here.
//!   - **Uom**: full generic CRUD — a leaf master with no cross-entity invariant (unique code is
//!     DB-enforced), safe to expose directly.

use std::sync::Arc;

use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, routing::{get, post}, Json, Router};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::BTreeMap;

use crate::application::service::catalog_write_service::{
    CatalogWriteError, CatalogWriteService, NewAttribute, NewAttributeValue, NewBrand, NewItem,
    NewItemGroup, NewItemVariant, NewUom, NewUomConversion,
};
use crate::CatalogModule;

use super::{
    create_attribute_read_routes, create_attribute_value_read_routes, create_brand_read_routes,
    create_item_group_read_routes, create_item_read_routes, create_item_variant_read_routes,
    create_uom_conversion_read_routes, create_uom_read_routes,
};

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}
#[derive(Debug, Serialize)]
struct IdResponse {
    id: Uuid,
}

fn err_response(e: CatalogWriteError) -> axum::response::Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(ErrorBody { error: e.code(), message: e.to_string() })).into_response()
}

// ── ItemGroup ─────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateItemGroupBody {
    code: String,
    name: String,
    #[serde(default)]
    parent_id: Option<Uuid>,
    #[serde(default)]
    is_group: bool,
}

async fn create_item_group(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateItemGroupBody>,
) -> axum::response::Response {
    match svc
        .create_item_group(NewItemGroup { code: b.code, name: b.name, parent_id: b.parent_id, is_group: b.is_group })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

// ── Item ──────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateItemBody {
    item_code: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    barcode: Option<String>,
    #[serde(default)]
    brand_id: Option<Uuid>,
    item_group_id: Uuid,
    default_uom_id: Uuid,
    #[serde(default)]
    item_type: Option<String>,
    #[serde(default = "default_true")]
    is_sales_item: bool,
    #[serde(default = "default_true")]
    is_purchase_item: bool,
    #[serde(default = "default_true")]
    is_stock_item: bool,
    #[serde(default)]
    hsn_code: Option<String>,
    #[serde(default = "default_true")]
    is_taxable: bool,
    #[serde(default)]
    weight_per_unit: Option<Decimal>,
    #[serde(default)]
    tags: Option<serde_json::Value>,
    #[serde(default)]
    data: Option<serde_json::Value>,
}
fn default_true() -> bool {
    true
}

async fn create_item(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateItemBody>,
) -> axum::response::Response {
    match svc
        .create_item(NewItem {
            item_code: b.item_code,
            name: b.name,
            description: b.description,
            barcode: b.barcode,
            brand_id: b.brand_id,
            item_group_id: b.item_group_id,
            default_uom_id: b.default_uom_id,
            item_type: b.item_type,
            is_sales_item: b.is_sales_item,
            is_purchase_item: b.is_purchase_item,
            is_stock_item: b.is_stock_item,
            hsn_code: b.hsn_code,
            is_taxable: b.is_taxable,
            weight_per_unit: b.weight_per_unit,
            tags: b.tags,
            data: b.data,
        })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

// ── UomConversion ───────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUomConversionBody {
    from_uom_id: Uuid,
    to_uom_id: Uuid,
    factor: Decimal,
}

async fn create_uom_conversion(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateUomConversionBody>,
) -> axum::response::Response {
    match svc
        .create_uom_conversion(NewUomConversion { from_uom_id: b.from_uom_id, to_uom_id: b.to_uom_id, factor: b.factor })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

// ── Attribute + AttributeValue ──────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAttributeBody {
    code: String,
    name: String,
    #[serde(default)]
    attribute_type: Option<String>,
}

async fn create_attribute(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateAttributeBody>,
) -> axum::response::Response {
    match svc
        .create_attribute(NewAttribute { code: b.code, name: b.name, attribute_type: b.attribute_type })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAttributeValueBody {
    attribute_id: Uuid,
    code: String,
    label: String,
    #[serde(default)]
    label_en: Option<String>,
    #[serde(default)]
    swatch_hex: Option<String>,
    #[serde(default)]
    sort_order: i32,
}

async fn create_attribute_value(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateAttributeValueBody>,
) -> axum::response::Response {
    match svc
        .create_attribute_value(NewAttributeValue {
            attribute_id: b.attribute_id,
            code: b.code,
            label: b.label,
            label_en: b.label_en,
            swatch_hex: b.swatch_hex,
            sort_order: b.sort_order,
        })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

// ── ItemVariant ─────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateItemVariantBody {
    item_id: Uuid,
    sku: String,
    #[serde(default)]
    variant_label: Option<String>,
    #[serde(default)]
    options: BTreeMap<String, String>,
    #[serde(default)]
    barcode: Option<String>,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    weight_per_unit: Option<Decimal>,
}

async fn create_item_variant(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateItemVariantBody>,
) -> axum::response::Response {
    match svc
        .create_item_variant(NewItemVariant {
            item_id: b.item_id,
            sku: b.sku,
            variant_label: b.variant_label,
            options: b.options,
            barcode: b.barcode,
            is_default: b.is_default,
            weight_per_unit: b.weight_per_unit,
        })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

// ── Uom + Brand (validated create; generic delete/patch are NOT mounted so a referenced
//    Uom/Brand can't be soft-deleted out from under items — council 2026-07-01) ──────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUomBody {
    code: String,
    name: String,
    #[serde(default)]
    uom_type: Option<String>,
    #[serde(default)]
    decimal_places: i32,
}

async fn create_uom(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateUomBody>,
) -> axum::response::Response {
    match svc
        .create_uom(NewUom { code: b.code, name: b.name, uom_type: b.uom_type, decimal_places: b.decimal_places })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBrandBody {
    code: String,
    name: String,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    logo_url: Option<String>,
    #[serde(default)]
    sort_order: i32,
}

async fn create_brand(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<CreateBrandBody>,
) -> axum::response::Response {
    match svc
        .create_brand(NewBrand {
            code: b.code,
            name: b.name,
            short_description: b.short_description,
            description: b.description,
            logo_url: b.logo_url,
            sort_order: b.sort_order,
        })
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteItemVariantBody {
    id: Uuid,
}

async fn delete_item_variant(
    State(svc): State<Arc<CatalogWriteService>>,
    Json(b): Json<DeleteItemVariantBody>,
) -> axum::response::Response {
    match svc.delete_item_variant(b.id).await {
        Ok(()) => (StatusCode::OK, Json(IdResponse { id: b.id })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Resolve a scanned barcode/SKU to a sellable item (scan → item, step 1 of the counter journey).
async fn lookup_item(State(svc): State<Arc<CatalogWriteService>>, Path(code): Path<String>) -> axum::response::Response {
    match svc.lookup_item(&code).await {
        Ok(Some(hit)) => (StatusCode::OK, Json(hit)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorBody { error: "item_not_found", message: format!("no item for code '{code}'") })).into_response(),
        Err(e) => err_response(e),
    }
}

fn create_catalog_write_routes(svc: Arc<CatalogWriteService>) -> Router {
    Router::new()
        .route("/item-groups", post(create_item_group))
        .route("/items", post(create_item))
        .route("/item-lookup/:code", get(lookup_item))
        .route("/uoms", post(create_uom))
        .route("/brands", post(create_brand))
        .route("/uom-conversions", post(create_uom_conversion))
        .route("/attributes", post(create_attribute))
        .route("/attribute-values", post(create_attribute_value))
        .route("/item-variants", post(create_item_variant))
        .route("/item-variants/delete", post(delete_item_variant))
        .with_state(svc)
}

/// Mount the catalog module with write paths locked to validated services.
/// **Prefer this over `CatalogModule::routes()` for any real deployment.**
pub fn create_guarded_catalog_routes(m: &CatalogModule) -> Router {
    Router::new()
        // Invariant-bearing entities: read + validated create only.
        .merge(create_item_read_routes(m.item_service.clone()))
        .merge(create_item_group_read_routes(m.item_group_service.clone()))
        .merge(create_uom_conversion_read_routes(m.uom_conversion_service.clone()))
        .merge(create_attribute_read_routes(m.attribute_service.clone()))
        .merge(create_attribute_value_read_routes(m.attribute_value_service.clone()))
        .merge(create_item_variant_read_routes(m.item_variant_service.clone()))
        // Uom + Brand: READ-ONLY generic + validated create. They are FK parents of Item /
        // UomConversion, so generic soft-delete/patch would orphan referencing rows (the FK never
        // fires on a soft delete, but every validated read filters deleted_at) — council 2026-07-01.
        .merge(create_uom_read_routes(m.uom_service.clone()))
        .merge(create_brand_read_routes(m.brand_service.clone()))
        // Validated writers for the locked entities.
        .merge(create_catalog_write_routes(m.catalog_write_service.clone()))
}
