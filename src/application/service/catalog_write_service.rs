//! Validated write path for Item, ItemGroup, and UomConversion — hand-authored (user-owned).
//!
//! Closes the CRUD-bypass: the generated 12-endpoint CRUD writes rows through `GenericCrudService`
//! with NO domain validation, so a well-formed request could create an Item pointing at a
//! non-existent item group or UOM, an Item that is neither sellable/purchasable/stocked, a
//! self-referential or non-positive UOM conversion, or an item-group whose parent is missing.
//!
//! `CatalogModule` mounts these validated writers via `create_guarded_catalog_routes`.
//!
//! All SQL lives in the repository newtypes (`item_repository.rs`, `item_group_repository.rs`,
//! `item_variant_repository.rs`, `uom_repository.rs`, `uom_conversion_repository.rs`,
//! `attribute_repository.rs`, `attribute_value_repository.rs`, `brand_repository.rs` — each
//! declared `user_owned` in `metaphor.codegen.yaml`). This service orchestrates the validated
//! writes: usage-flag checks, FK existence probes, unique-constraint disambiguation, and the
//! in-tx variant lifecycle (`has_variants` flag flips + soft-delete).

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::entity::CatalogStatus;

// Re-export `ItemHit` so the service's public API surface (`application::service::ItemHit`) stays
// stable now that the type itself lives next to the SQL that produces it.
pub use crate::infrastructure::persistence::ItemHit;
use crate::infrastructure::persistence::{
    AttributeRepository, AttributeValueRepository, BrandRepository, ItemGroupRepository,
    ItemRepository, ItemVariantRepository, NewAttributeRow, NewAttributeValueRow, NewBrandRow,
    NewItemGroupRow, NewItemRow, NewItemVariantRow, NewUomConversionRow, NewUomRow,
    UomConversionRepository, UomRepository,
};

#[derive(Debug)]
pub enum CatalogWriteError {
    ItemGroupNotFound(Uuid),
    UomNotFound(Uuid),
    ParentNotFound(Uuid),
    NoUsageFlag,
    SameUom,
    NonPositiveFactor,
    DuplicateItemCode(String),
    DuplicateBarcode(String),
    DuplicateConversion,
    // Attributes & variants
    AttributeNotFound(Uuid),
    BrandNotFound(Uuid),
    ItemNotFound(Uuid),
    ItemVariantNotFound(Uuid),
    /// A status transition the CatalogStatus state machine does not permit (e.g.
    /// `discontinued → active` — `discontinued` is terminal). See schema/hooks/catalog.hook.yaml.
    InvalidStatusTransition { from: CatalogStatus, to: CatalogStatus },
    DuplicateUomCode(String),
    DuplicateBrandCode(String),
    DuplicateAttributeCode(String),
    DuplicateValueCode(String),
    DuplicateSku(String),
    NoOptions,
    UnknownAttribute(String),
    UnknownAttributeValue(String),
    /// A write path needed the caller's company but the request scope was unset
    /// (missing `with_company_scope` / `with_request_scope` middleware).
    NoCompanyScope,
    Db(sqlx::Error),
}

impl CatalogWriteError {
    pub fn code(&self) -> &'static str {
        match self {
            CatalogWriteError::ItemGroupNotFound(_) => "item_group_not_found",
            CatalogWriteError::UomNotFound(_) => "uom_not_found",
            CatalogWriteError::ParentNotFound(_) => "parent_not_found",
            CatalogWriteError::NoUsageFlag => "no_usage_flag",
            CatalogWriteError::SameUom => "same_uom",
            CatalogWriteError::NonPositiveFactor => "non_positive_factor",
            CatalogWriteError::DuplicateItemCode(_) => "duplicate_item_code",
            CatalogWriteError::DuplicateBarcode(_) => "duplicate_barcode",
            CatalogWriteError::DuplicateConversion => "duplicate_conversion",
            CatalogWriteError::AttributeNotFound(_) => "attribute_not_found",
            CatalogWriteError::BrandNotFound(_) => "brand_not_found",
            CatalogWriteError::ItemNotFound(_) => "item_not_found",
            CatalogWriteError::ItemVariantNotFound(_) => "item_variant_not_found",
            CatalogWriteError::InvalidStatusTransition { .. } => "invalid_status_transition",
            CatalogWriteError::DuplicateUomCode(_) => "duplicate_uom_code",
            CatalogWriteError::DuplicateBrandCode(_) => "duplicate_brand_code",
            CatalogWriteError::DuplicateAttributeCode(_) => "duplicate_attribute_code",
            CatalogWriteError::DuplicateValueCode(_) => "duplicate_value_code",
            CatalogWriteError::DuplicateSku(_) => "duplicate_sku",
            CatalogWriteError::NoOptions => "no_options",
            CatalogWriteError::UnknownAttribute(_) => "unknown_attribute",
            CatalogWriteError::UnknownAttributeValue(_) => "unknown_attribute_value",
            CatalogWriteError::NoCompanyScope => "no_company_scope",
            CatalogWriteError::Db(_) => "internal_error",
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            CatalogWriteError::Db(_) => 500,
            CatalogWriteError::NoCompanyScope => 401,
            _ => 422,
        }
    }
}
impl std::fmt::Display for CatalogWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())?;
        match self {
            CatalogWriteError::ItemGroupNotFound(id)
            | CatalogWriteError::UomNotFound(id)
            | CatalogWriteError::ParentNotFound(id) => write!(f, ": {id}"),
            CatalogWriteError::DuplicateItemCode(v)
            | CatalogWriteError::DuplicateBarcode(v)
            | CatalogWriteError::DuplicateAttributeCode(v)
            | CatalogWriteError::DuplicateValueCode(v)
            | CatalogWriteError::DuplicateSku(v)
            | CatalogWriteError::DuplicateUomCode(v)
            | CatalogWriteError::DuplicateBrandCode(v)
            | CatalogWriteError::UnknownAttribute(v)
            | CatalogWriteError::UnknownAttributeValue(v) => write!(f, ": {v}"),
            CatalogWriteError::AttributeNotFound(id)
            | CatalogWriteError::BrandNotFound(id)
            | CatalogWriteError::ItemNotFound(id)
            | CatalogWriteError::ItemVariantNotFound(id) => write!(f, ": {id}"),
            CatalogWriteError::InvalidStatusTransition { from, to } => write!(f, ": {from:?} -> {to:?}"),
            _ => Ok(()),
        }
    }
}
impl std::error::Error for CatalogWriteError {}
impl From<sqlx::Error> for CatalogWriteError {
    fn from(e: sqlx::Error) -> Self {
        CatalogWriteError::Db(e)
    }
}

#[derive(Debug, Clone)]
pub struct NewItemGroup {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub is_group: bool,
}

#[derive(Debug, Clone)]
pub struct NewItem {
    pub company_id: Uuid,
    pub item_code: String,
    pub name: String,
    pub description: Option<String>,
    pub barcode: Option<String>,
    pub brand_id: Option<Uuid>,
    pub item_group_id: Uuid,
    pub default_uom_id: Uuid,
    pub item_type: Option<String>,
    pub is_sales_item: bool,
    pub is_purchase_item: bool,
    pub is_stock_item: bool,
    pub hsn_code: Option<String>,
    pub is_taxable: bool,
    pub weight_per_unit: Option<Decimal>,
    pub tags: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}

/// Physical (stockable-capable) item types. Non-physical types are never stockable.
pub fn is_physical_item_type(item_type: &str) -> bool {
    matches!(item_type, "physical_good" | "bundle" | "rental")
}

#[derive(Debug, Clone)]
pub struct NewUomConversion {
    pub company_id: Uuid,
    pub from_uom_id: Uuid,
    pub to_uom_id: Uuid,
    pub factor: Decimal,
}

#[derive(Debug, Clone)]
pub struct NewAttribute {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub attribute_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAttributeValue {
    pub company_id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
    pub label: String,
    pub label_en: Option<String>,
    pub swatch_hex: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct NewUom {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub uom_type: Option<String>,
    pub decimal_places: i32,
}

#[derive(Debug, Clone)]
pub struct NewBrand {
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub short_description: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct NewItemVariant {
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub sku: String,
    pub variant_label: Option<String>,
    /// `{attribute_code: value_code}` — validated against the Attribute registry.
    pub options: std::collections::BTreeMap<String, String>,
    pub barcode: Option<String>,
    pub is_default: bool,
    pub weight_per_unit: Option<Decimal>,
}

#[derive(Clone)]
pub struct CatalogWriteService {
    db_pool: PgPool,
}

impl CatalogWriteService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Resolve a scanned code (barcode OR SKU/item_code) to a sellable identity. Matches the base item
    /// first (by `barcode` or `item_code`), then a variant (by `barcode` or `sku`). `None` = unknown
    /// code. Per-company (ADR-0010 B1): the caller's company (from `current_company()`, set by the
    /// request scope) is bound into the lookup as defense-in-depth on top of the RLS fence, so a
    /// missed scope still returns nothing instead of leaking another tenant's item.
    pub async fn lookup_item(&self, code: &str) -> Result<Option<ItemHit>, CatalogWriteError> {
        let company = company_scope::current_company();
        let items = ItemRepository::new(self.db_pool.clone());
        if let Some(hit) = items
            .find_by_scan_code(&self.db_pool, code, company)
            .await?
        {
            return Ok(Some(hit));
        }
        let variants = ItemVariantRepository::new(self.db_pool.clone());
        let hit = variants
            .find_variant_by_scan_code(&self.db_pool, code, company)
            .await?;
        Ok(hit)
    }

    fn is_dup(e: &sqlx::Error, needle: &str) -> bool {
        e.as_database_error()
            .map(|d| d.is_unique_violation() && d.constraint().unwrap_or("").contains(needle))
            .unwrap_or(false)
    }

    pub async fn create_item_group(&self, g: NewItemGroup) -> Result<Uuid, CatalogWriteError> {
        let company = g.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let item_groups = ItemGroupRepository::new(self.db_pool.clone());
            if let Some(pid) = g.parent_id {
                if !item_groups.exists_id_in_company(&self.db_pool, pid, company).await? {
                    return Err(CatalogWriteError::ParentNotFound(pid));
                }
            }
            let id = Uuid::new_v4();
            let r = item_groups
                .insert_item_group(
                    &self.db_pool,
                    &NewItemGroupRow {
                        id,
                        company_id: company,
                        code: &g.code,
                        name: &g.name,
                        parent_id: g.parent_id,
                        is_group: g.is_group,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "code") => {
                    Err(CatalogWriteError::DuplicateItemCode(g.code))
                }
                Err(e) => Err(e.into()),
            }
        }).await
    }

    pub async fn create_item(&self, i: NewItem) -> Result<Uuid, CatalogWriteError> {
        let company = i.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let item_type = i.item_type.clone().unwrap_or_else(|| "physical_good".to_string());
            // Non-physical types (digital/service/subscription/gift_card) are never stockable —
            // derive it from the type rather than trusting the caller's flag.
            let is_stock_item = i.is_stock_item && is_physical_item_type(&item_type);
            if !(i.is_sales_item || i.is_purchase_item || is_stock_item) {
                return Err(CatalogWriteError::NoUsageFlag);
            }
            let item_groups = ItemGroupRepository::new(self.db_pool.clone());
            if !item_groups.exists_id_in_company(&self.db_pool, i.item_group_id, company).await? {
                return Err(CatalogWriteError::ItemGroupNotFound(i.item_group_id));
            }
            let uoms = UomRepository::new(self.db_pool.clone());
            if !uoms.exists_id_in_company(&self.db_pool, i.default_uom_id, company).await? {
                return Err(CatalogWriteError::UomNotFound(i.default_uom_id));
            }
            if let Some(bid) = i.brand_id {
                let brands = BrandRepository::new(self.db_pool.clone());
                if !brands.exists_id_in_company(&self.db_pool, bid, company).await? {
                    return Err(CatalogWriteError::BrandNotFound(bid));
                }
            }
            let id = Uuid::new_v4();
            let tags = i.tags.clone().unwrap_or_else(|| serde_json::json!([]));
            let data = i.data.clone().unwrap_or_else(|| serde_json::json!({}));
            let items = ItemRepository::new(self.db_pool.clone());
            let r = items
                .insert_item(
                    &self.db_pool,
                    &NewItemRow {
                        id,
                        company_id: company,
                        item_code: &i.item_code,
                        name: &i.name,
                        description: i.description.as_deref(),
                        barcode: i.barcode.as_deref(),
                        brand_id: i.brand_id,
                        item_group_id: i.item_group_id,
                        default_uom_id: i.default_uom_id,
                        item_type: &item_type,
                        is_sales_item: i.is_sales_item,
                        is_purchase_item: i.is_purchase_item,
                        is_stock_item,
                        hsn_code: i.hsn_code.as_deref(),
                        is_taxable: i.is_taxable,
                        weight_per_unit: i.weight_per_unit,
                        tags: &tags,
                        data: &data,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "barcode") => Err(CatalogWriteError::DuplicateBarcode(
                    i.barcode.unwrap_or_default(),
                )),
                Err(e) if Self::is_dup(&e, "item_code") || Self::is_dup(&e, "items") => {
                    Err(CatalogWriteError::DuplicateItemCode(i.item_code))
                }
                Err(e) => Err(e.into()),
            }
        }).await
    }

    pub async fn create_uom_conversion(
        &self,
        c: NewUomConversion,
    ) -> Result<Uuid, CatalogWriteError> {
        let company = c.company_id;
        company_scope::with_company_scope(Some(company), async move {
            if c.from_uom_id == c.to_uom_id {
                return Err(CatalogWriteError::SameUom);
            }
            if c.factor <= Decimal::ZERO {
                return Err(CatalogWriteError::NonPositiveFactor);
            }
            let uoms = UomRepository::new(self.db_pool.clone());
            if !uoms.exists_id_in_company(&self.db_pool, c.from_uom_id, company).await? {
                return Err(CatalogWriteError::UomNotFound(c.from_uom_id));
            }
            if !uoms.exists_id_in_company(&self.db_pool, c.to_uom_id, company).await? {
                return Err(CatalogWriteError::UomNotFound(c.to_uom_id));
            }
            let id = Uuid::new_v4();
            let repo = UomConversionRepository::new(self.db_pool.clone());
            let r = repo
                .insert_uom_conversion(
                    &self.db_pool,
                    &NewUomConversionRow {
                        id,
                        company_id: company,
                        from_uom_id: c.from_uom_id,
                        to_uom_id: c.to_uom_id,
                        factor: c.factor,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) => {
                    Err(CatalogWriteError::DuplicateConversion)
                }
                Err(e) => Err(e.into()),
            }
        }).await
    }

    pub async fn create_attribute(&self, a: NewAttribute) -> Result<Uuid, CatalogWriteError> {
        let company = a.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let id = Uuid::new_v4();
            let at = a.attribute_type.clone().unwrap_or_else(|| "other".to_string());
            let repo = AttributeRepository::new(self.db_pool.clone());
            let r = repo
                .insert_attribute(
                    &self.db_pool,
                    &NewAttributeRow {
                        id,
                        company_id: company,
                        code: &a.code,
                        name: &a.name,
                        attribute_type: &at,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateAttributeCode(a.code)),
                Err(e) => Err(e.into()),
            }
        }).await
    }

    pub async fn create_attribute_value(&self, v: NewAttributeValue) -> Result<Uuid, CatalogWriteError> {
        let company = v.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let attrs = AttributeRepository::new(self.db_pool.clone());
            if !attrs.exists_id_in_company(&self.db_pool, v.attribute_id, company).await? {
                return Err(CatalogWriteError::AttributeNotFound(v.attribute_id));
            }
            let id = Uuid::new_v4();
            let repo = AttributeValueRepository::new(self.db_pool.clone());
            let r = repo
                .insert_attribute_value(
                    &self.db_pool,
                    &NewAttributeValueRow {
                        id,
                        company_id: company,
                        attribute_id: v.attribute_id,
                        code: &v.code,
                        label: &v.label,
                        label_en: v.label_en.as_deref(),
                        swatch_hex: v.swatch_hex.as_deref(),
                        sort_order: v.sort_order,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateValueCode(v.code)),
                Err(e) => Err(e.into()),
            }
        }).await
    }

    /// Create a variant SKU. Validates the item exists, every option maps to a known
    /// attribute+value in the registry, then persists the variant and flips the item's
    /// `has_variants` flag. `variant_label` defaults to the option value labels joined " / ".
    pub async fn create_item_variant(&self, v: NewItemVariant) -> Result<Uuid, CatalogWriteError> {
        let company = v.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let items = ItemRepository::new(self.db_pool.clone());
            if !items.exists_id_in_company(&self.db_pool, v.item_id, company).await? {
                return Err(CatalogWriteError::ItemNotFound(v.item_id));
            }
            if v.options.is_empty() {
                return Err(CatalogWriteError::NoOptions);
            }

            // Validate options against the registry and collect display labels for the label default.
            // The registry lookups are company-scoped (defense-in-depth on top of RLS) so a cross-tenant
            // collision on attribute code never bleeds into this variant's validation.
            let attr_values = AttributeValueRepository::new(self.db_pool.clone());
            let attrs = AttributeRepository::new(self.db_pool.clone());
            let mut labels: Vec<String> = Vec::with_capacity(v.options.len());
            for (attr_code, val_code) in &v.options {
                let row = attr_values
                    .find_value_with_attribute(&self.db_pool, attr_code, val_code, company)
                    .await?;
                match row {
                    Some(r) => labels.push(r.label),
                    None => {
                        // Distinguish unknown axis vs unknown value for a clearer error.
                        let attr_ok = attrs.find_id_by_code(&self.db_pool, attr_code, company).await?;
                        return if attr_ok.is_some() {
                            Err(CatalogWriteError::UnknownAttributeValue(format!("{attr_code}={val_code}")))
                        } else {
                            Err(CatalogWriteError::UnknownAttribute(attr_code.clone()))
                        };
                    }
                }
            }

            let label = v.variant_label.clone().unwrap_or_else(|| labels.join(" / "));
            let options_json = serde_json::to_value(&v.options).unwrap_or(serde_json::json!({}));

            let id = Uuid::new_v4();
            let mut tx = self.db_pool.begin().await?;
            // Bind the company onto this transaction so the RLS WITH CHECK accepts the row
            // (ADR-0008 pattern for hand-written write services managing their own tx).
            company_scope::bind_current_company(&mut tx).await?;
            let variants = ItemVariantRepository::new(self.db_pool.clone());
            let r = variants
                .insert_variant(
                    &mut *tx,
                    &NewItemVariantRow {
                        id,
                        company_id: company,
                        item_id: v.item_id,
                        sku: &v.sku,
                        variant_label: &label,
                        options: &options_json,
                        barcode: v.barcode.as_deref(),
                        is_default: v.is_default,
                        weight_per_unit: v.weight_per_unit,
                    },
                )
                .await;
            if let Err(e) = r {
                drop(tx);
                return if Self::is_dup(&e, "barcode") {
                    Err(CatalogWriteError::DuplicateBarcode(v.barcode.unwrap_or_default()))
                } else if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) {
                    Err(CatalogWriteError::DuplicateSku(v.sku))
                } else {
                    Err(e.into())
                };
            }
            items.set_has_variants_true(&mut *tx, v.item_id).await?;
            tx.commit().await?;
            Ok(id)
        }).await
    }

    /// Soft-delete a variant and keep `Item.has_variants` honest: if the item has no live variants
    /// left, flip the flag back to false so the storefront picker never lies. Company-scoped: the
    /// caller's company (from the request scope) filters the lookup AND binds into the transaction so
    /// the RLS WITH CHECK accepts the soft-delete write.
    pub async fn delete_item_variant(&self, variant_id: Uuid) -> Result<(), CatalogWriteError> {
        let company = company_scope::current_company()
            .ok_or(CatalogWriteError::NoCompanyScope)?;
        let variants = ItemVariantRepository::new(self.db_pool.clone());
        let item_id = variants
            .find_item_id_for_live(&self.db_pool, variant_id, company)
            .await?
            .ok_or(CatalogWriteError::ItemVariantNotFound(variant_id))?;

        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_current_company(&mut tx).await?;
        variants.soft_delete_variant(&mut *tx, variant_id, company).await?;
        let remaining = variants.count_live_variants(&mut *tx, item_id, company).await?;
        if remaining == 0 {
            let items = ItemRepository::new(self.db_pool.clone());
            items.set_has_variants_false(&mut *tx, item_id).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Transition an Item's lifecycle status, enforcing the CatalogStatus state machine declared
    /// in schema/hooks/catalog.hook.yaml (`active ↔ inactive`, `active|inactive → discontinued`;
    /// `discontinued` is terminal). Company-scoped via the request scope: `current_company()` gates
    /// the call (401 if unset), and `bind_current_company` scopes the lookup + write through RLS.
    pub async fn transition_item_status(
        &self,
        item_id: Uuid,
        target: CatalogStatus,
    ) -> Result<(), CatalogWriteError> {
        if company_scope::current_company().is_none() {
            return Err(CatalogWriteError::NoCompanyScope);
        }
        let items = ItemRepository::new(self.db_pool.clone());
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_current_company(&mut tx).await?;
        let current = items
            .find_status(&mut *tx, item_id)
            .await?
            .ok_or(CatalogWriteError::ItemNotFound(item_id))?;
        if !Self::transition_allowed(current, target) {
            return Err(CatalogWriteError::InvalidStatusTransition { from: current, to: target });
        }
        items.set_status(&mut *tx, item_id, target).await?;
        tx.commit().await?;
        Ok(())
    }

    /// The CatalogStatus state machine (schema/hooks/catalog.hook.yaml): `discontinued` is terminal.
    fn transition_allowed(from: CatalogStatus, to: CatalogStatus) -> bool {
        use CatalogStatus::*;
        matches!(
            (from, to),
            (Active, Inactive) | (Inactive, Active) | (Active, Discontinued) | (Inactive, Discontinued)
        )
    }

    /// Create a Uom (leaf master). Validated create so the guarded surface can mount Uom read-only
    /// (generic delete/patch would orphan items that FK-point at it — council 2026-07-01).
    pub async fn create_uom(&self, u: NewUom) -> Result<Uuid, CatalogWriteError> {
        let company = u.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let id = Uuid::new_v4();
            let ut = u.uom_type.clone().unwrap_or_else(|| "count".to_string());
            let repo = UomRepository::new(self.db_pool.clone());
            let r = repo
                .insert_uom(
                    &self.db_pool,
                    &NewUomRow {
                        id,
                        company_id: company,
                        code: &u.code,
                        name: &u.name,
                        uom_type: &ut,
                        decimal_places: u.decimal_places,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateUomCode(u.code)),
                Err(e) => Err(e.into()),
            }
        }).await
    }

    /// Create a Brand (leaf master). Validated create — same rationale as `create_uom`.
    pub async fn create_brand(&self, b: NewBrand) -> Result<Uuid, CatalogWriteError> {
        let company = b.company_id;
        company_scope::with_company_scope(Some(company), async move {
            let id = Uuid::new_v4();
            let repo = BrandRepository::new(self.db_pool.clone());
            let r = repo
                .insert_brand(
                    &self.db_pool,
                    &NewBrandRow {
                        id,
                        company_id: company,
                        code: &b.code,
                        name: &b.name,
                        short_description: b.short_description.as_deref(),
                        description: b.description.as_deref(),
                        logo_url: b.logo_url.as_deref(),
                        sort_order: b.sort_order,
                    },
                )
                .await;
            match r {
                Ok(_) => Ok(id),
                Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateBrandCode(b.code)),
                Err(e) => Err(e.into()),
            }
        }).await
    }
}
