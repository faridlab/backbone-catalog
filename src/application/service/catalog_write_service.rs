//! Validated write path for Item, ItemGroup, and the UoM tree — hand-authored (user-owned).
//!
//! Closes the CRUD-bypass: the generated 12-endpoint CRUD writes rows through `GenericCrudService`
//! with NO domain validation, so a well-formed request could create an Item pointing at a
//! non-existent item group or UOM, an Item that is neither sellable/purchasable/stocked, or an
//! item-group whose parent is missing.
//!
//! Units of measure form parent-store reference trees (ADR-0023): a unit either is a tree root
//! or points at a reference unit with a positive ratio, and every unit carries a recursive stored
//! factor to its tree's root that this service re-derives on every tree write. Conversion between
//! units is application-side math over those stored factors (`convert_quantity`) and fails loudly
//! with a typed error when the two units live in different trees. The pre-v19 pairwise
//! conversion-table write path is retired by that rewrite: the `uom_conversions` table and its
//! read surface remain for legacy data and future item-specific layering, but it is no longer an
//! input to conversion.
//!
//! Tenant-agnostic (ADR-0029): the service carries no tenancy of its own. Statements run plain
//! on the module pool; when the composing host mounts a request-scoped org fence
//! (`backbone_orm::org_scope::with_org_request_scope`), plain pool reads and writes ride the
//! request-dedicated scoped connection and only in-scope rows are visible. Transactions this
//! service opens itself re-bind the caller's ambient org scope when one is present
//! ([`CatalogWriteService::relay_ambient_scope`]) and stay plain otherwise, so the module
//! functions undecorated (standalone deployment, background jobs).
//!
//! `CatalogModule` mounts these validated writers via `create_guarded_catalog_routes`.
//!
//! All SQL lives in the repository newtypes (`item_repository.rs`, `item_group_repository.rs`,
//! `item_variant_repository.rs`, `uom_repository.rs`, `attribute_repository.rs`,
//! `attribute_value_repository.rs`, `brand_repository.rs` — each declared `user_owned` in
//! `metaphor.codegen.yaml`). This service orchestrates the validated writes: usage-flag checks,
//! FK existence probes, unique-constraint disambiguation, the in-tx variant lifecycle
//! (`has_variants` flag flips + soft-delete), and the UoM tree link + factor re-derivation.

use backbone_orm::org_scope;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::entity::CatalogStatus;
use crate::domain::services::uom_tree::{
    convert_quantity, ConversionRounding, UomChain, UomConversionError,
};

// Re-export `ItemHit` so the service's public API surface (`application::service::ItemHit`) stays
// stable now that the type itself lives next to the SQL that produces it.
pub use crate::infrastructure::persistence::ItemHit;
use crate::infrastructure::persistence::{
    AttributeRepository, AttributeValueRepository, BrandRepository, ItemGroupRepository,
    ItemRepository, ItemVariantRepository, NewAttributeRow, NewAttributeValueRow, NewBrandRow,
    NewItemGroupRow, NewItemRow, NewItemVariantRow, NewUomRow, UomRepository,
};

#[derive(Debug)]
pub enum CatalogWriteError {
    ItemGroupNotFound(Uuid),
    UomNotFound(Uuid),
    ParentNotFound(Uuid),
    NoUsageFlag,
    DuplicateItemCode(String),
    DuplicateBarcode(String),
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
    /// `relative_factor` was supplied without `relative_uom_id` (or vice versa): the tree
    /// link shape is exactly "both set" or "both unset" (ADR-0023).
    RelativeShapeMismatch,
    /// A tree link ratio of zero or less (the stored factor chain must stay positive).
    NonPositiveRelativeFactor,
    /// Re-parenting a unit onto itself or one of its own descendants — that link would
    /// form a cycle, so no root (and no derivable factor) exists anymore.
    UomCycle { parent: Uuid },
    /// A conversion between units of different trees (or over a corrupt tree) failed
    /// loudly — the typed ADR-0023 failure, never a silent numeric result.
    Conversion(UomConversionError),
    /// The unit is module-seeded reference data (`is_protected`) and cannot be deleted;
    /// retire it through the status lifecycle instead (UM-4).
    ProtectedUom { code: String },
    /// The unit is still the live parent of live units — re-link or detach the children
    /// before retiring it, or the tree would lean on an archived reference.
    UomHasChildren { code: String },
    /// A live item still uses the unit as its default unit of measure (the orphaning
    /// hazard that keeps generic delete off the guarded surface — ADR-005).
    UomInUse { code: String },
    Db(sqlx::Error),
}

impl CatalogWriteError {
    pub fn code(&self) -> &'static str {
        match self {
            CatalogWriteError::ItemGroupNotFound(_) => "item_group_not_found",
            CatalogWriteError::UomNotFound(_) => "uom_not_found",
            CatalogWriteError::ParentNotFound(_) => "parent_not_found",
            CatalogWriteError::NoUsageFlag => "no_usage_flag",
            CatalogWriteError::DuplicateItemCode(_) => "duplicate_item_code",
            CatalogWriteError::DuplicateBarcode(_) => "duplicate_barcode",
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
            CatalogWriteError::RelativeShapeMismatch => "relative_shape_mismatch",
            CatalogWriteError::NonPositiveRelativeFactor => "non_positive_relative_factor",
            CatalogWriteError::UomCycle { .. } => "uom_cycle",
            CatalogWriteError::Conversion(e) => e.code(),
            CatalogWriteError::ProtectedUom { .. } => "protected_uom",
            CatalogWriteError::UomHasChildren { .. } => "uom_has_children",
            CatalogWriteError::UomInUse { .. } => "uom_in_use",
            CatalogWriteError::Db(_) => "internal_error",
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            CatalogWriteError::Db(_) => 500,
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
            CatalogWriteError::UomCycle { parent } => write!(f, ": {parent}"),
            CatalogWriteError::Conversion(e) => write!(f, ": {e}"),
            CatalogWriteError::ProtectedUom { code }
            | CatalogWriteError::UomHasChildren { code }
            | CatalogWriteError::UomInUse { code } => write!(f, ": {code}"),
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
    pub code: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub is_group: bool,
}

#[derive(Debug, Clone)]
pub struct NewItem {
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
    pub standard_cost: Option<Decimal>,
    pub tags: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}

/// Physical (stockable-capable) item types. Non-physical types are never stockable.
pub fn is_physical_item_type(item_type: &str) -> bool {
    matches!(item_type, "physical_good" | "bundle" | "rental")
}

#[derive(Debug, Clone)]
pub struct NewAttribute {
    pub code: String,
    pub name: String,
    pub attribute_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAttributeValue {
    pub attribute_id: Uuid,
    pub code: String,
    pub label: String,
    pub label_en: Option<String>,
    pub swatch_hex: Option<String>,
    pub sort_order: i32,
}

/// A new unit of measure. Leaving `relative_uom_id`/`relative_factor` unset creates a
/// tree ROOT; setting exactly one of the two is a shape error (`RelativeShapeMismatch`).
/// The stored `factor` is derived by the service (parent's factor × relative_factor),
/// never supplied here.
#[derive(Debug, Clone)]
pub struct NewUom {
    pub code: String,
    pub name: String,
    pub uom_type: Option<String>,
    pub decimal_places: i32,
    /// Parent (reference) unit — `None` makes this unit a tree root.
    pub relative_uom_id: Option<Uuid>,
    /// Ratio to the parent: 1 of the new unit = `relative_factor` of the parent.
    pub relative_factor: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct NewBrand {
    pub code: String,
    pub name: String,
    pub short_description: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct NewItemVariant {
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

    /// Re-bind the caller's ambient org scope onto a transaction this service opened itself.
    ///
    /// Reads and writes on the shared pool already ride the request-dedicated scoped
    /// connection when the composed host mounts one; a transaction begun on that pool would
    /// otherwise miss the fence variables, so the ambient scope (if any) is re-set
    /// transaction-locally. With no ambient scope (standalone deployment, background jobs)
    /// the transaction stays plain — the module functions undecorated.
    async fn relay_ambient_scope(conn: &mut sqlx::PgConnection) -> Result<(), sqlx::Error> {
        if let Some(scope) = org_scope::current_org_scope() {
            org_scope::bind_org_scope_on(conn, &scope).await?;
        }
        Ok(())
    }

    /// Resolve a scanned code (barcode OR SKU/item_code) to a sellable identity. Matches the base item
    /// first (by `barcode` or `item_code`), then a variant (by `barcode` or `sku`). `None` = unknown
    /// code. Tenant-agnostic: under a composed host's org request scope the lookups ride the
    /// request-dedicated scoped connection, so out-of-scope rows are simply not found.
    pub async fn lookup_item(&self, code: &str) -> Result<Option<ItemHit>, CatalogWriteError> {
        let items = ItemRepository::new(self.db_pool.clone());
        if let Some(hit) = items.find_by_scan_code(&self.db_pool, code).await? {
            return Ok(Some(hit));
        }
        let variants = ItemVariantRepository::new(self.db_pool.clone());
        let hit = variants.find_variant_by_scan_code(&self.db_pool, code).await?;
        Ok(hit)
    }

    fn is_dup(e: &sqlx::Error, needle: &str) -> bool {
        e.as_database_error()
            .map(|d| d.is_unique_violation() && d.constraint().unwrap_or("").contains(needle))
            .unwrap_or(false)
    }

    pub async fn create_item_group(&self, g: NewItemGroup) -> Result<Uuid, CatalogWriteError> {
        let item_groups = ItemGroupRepository::new(self.db_pool.clone());
        if let Some(pid) = g.parent_id {
            if !item_groups.exists_id(&self.db_pool, pid).await? {
                return Err(CatalogWriteError::ParentNotFound(pid));
            }
        }
        let id = Uuid::new_v4();
        let r = item_groups
            .insert_item_group(
                &self.db_pool,
                &NewItemGroupRow {
                    id,
                    code: &g.code,
                    name: &g.name,
                    parent_id: g.parent_id,
                    is_group: g.is_group,
                },
            )
            .await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateItemCode(g.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_item(&self, i: NewItem) -> Result<Uuid, CatalogWriteError> {
        let item_type = i.item_type.clone().unwrap_or_else(|| "physical_good".to_string());
        // Non-physical types (digital/service/subscription/gift_card) are never stockable —
        // derive it from the type rather than trusting the caller's flag.
        let is_stock_item = i.is_stock_item && is_physical_item_type(&item_type);
        if !(i.is_sales_item || i.is_purchase_item || is_stock_item) {
            return Err(CatalogWriteError::NoUsageFlag);
        }
        let item_groups = ItemGroupRepository::new(self.db_pool.clone());
        if !item_groups.exists_id(&self.db_pool, i.item_group_id).await? {
            return Err(CatalogWriteError::ItemGroupNotFound(i.item_group_id));
        }
        let uoms = UomRepository::new(self.db_pool.clone());
        if !uoms.exists_id(&self.db_pool, i.default_uom_id).await? {
            return Err(CatalogWriteError::UomNotFound(i.default_uom_id));
        }
        if let Some(bid) = i.brand_id {
            let brands = BrandRepository::new(self.db_pool.clone());
            if !brands.exists_id(&self.db_pool, bid).await? {
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
                    standard_cost: i.standard_cost,
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
    }

    /// Create a Uom (leaf master) on the parent-store tree (ADR-0023). Validated create so
    /// the guarded surface can mount Uom read-only (generic delete/patch would orphan items
    /// that FK-point at it — council 2026-07-01). With `relative_uom_id` set the unit becomes
    /// a child of that reference unit and its stored factor is derived as
    /// `parent.factor * relative_factor`; without it the unit is a new tree root (factor 1).
    pub async fn create_uom(&self, u: NewUom) -> Result<Uuid, CatalogWriteError> {
        let (relative_uom_id, relative_factor, factor) = match (u.relative_uom_id, u.relative_factor) {
            (None, None) => (None, None, Decimal::ONE),
            (Some(parent), Some(rf)) => {
                if rf <= Decimal::ZERO {
                    return Err(CatalogWriteError::NonPositiveRelativeFactor);
                }
                let uoms = UomRepository::new(self.db_pool.clone());
                if !uoms.exists_id(&self.db_pool, parent).await? {
                    return Err(CatalogWriteError::ParentNotFound(parent));
                }
                // A new unit has no descendants yet, so its stored factor is exactly
                // the parent's stored factor scaled by the link ratio.
                let parent_factor = uoms
                    .find_factor(&self.db_pool, parent)
                    .await?
                    .ok_or(CatalogWriteError::ParentNotFound(parent))?;
                (Some(parent), Some(rf), parent_factor * rf)
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(CatalogWriteError::RelativeShapeMismatch);
            }
        };
        let id = Uuid::new_v4();
        let ut = u.uom_type.clone().unwrap_or_else(|| "count".to_string());
        let repo = UomRepository::new(self.db_pool.clone());
        let r = repo
            .insert_uom(
                &self.db_pool,
                &NewUomRow {
                    id,
                    code: &u.code,
                    name: &u.name,
                    uom_type: &ut,
                    decimal_places: u.decimal_places,
                    relative_uom_id,
                    relative_factor,
                    factor,
                },
            )
            .await;
        match r {
            Ok(_) => {
                // A tree write ends with the declared derive: re-derive every stored
                // factor from the roots (heals any drift the new row would otherwise
                // inherit from a tampered parent, and raises loudly if any chain is
                // unreachable from a root).
                repo.recompute_factors(&self.db_pool).await?;
                Ok(id)
            }
            Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateUomCode(u.code)),
            Err(e) => Err(e.into()),
        }
    }

    /// Re-parent a live unit onto a new reference unit (or detach it to become a tree root
    /// by passing `None`). After the link changes, every stored factor in the affected tree
    /// is re-derived (the unit's own and all its descendants'). Guards:
    /// shape (both fields together), positivity, parent existence, and the
    /// cycle rule — a unit may never point at itself or its own descendant.
    pub async fn set_uom_relative(
        &self,
        uom_id: Uuid,
        relative: Option<(Uuid, Decimal)>,
    ) -> Result<(), CatalogWriteError> {
        let (relative_uom_id, relative_factor) = match relative {
            None => (None, None),
            Some((parent, rf)) => {
                if rf <= Decimal::ZERO {
                    return Err(CatalogWriteError::NonPositiveRelativeFactor);
                }
                (Some(parent), Some(rf))
            }
        };
        let uoms = UomRepository::new(self.db_pool.clone());
        // Pre-validation reads run on the pool before the transaction opens; the mutation
        // and the factor re-derivation below share one committed unit of work.
        if !uoms.exists_id(&self.db_pool, uom_id).await? {
            return Err(CatalogWriteError::UomNotFound(uom_id));
        }
        let mut tx = self.db_pool.begin().await?;
        Self::relay_ambient_scope(&mut tx).await?;
        if let Some(parent) = relative_uom_id {
            if !uoms.exists_id(&self.db_pool, parent).await? {
                return Err(CatalogWriteError::ParentNotFound(parent));
            }
            // The cycle rule: linking at yourself or any descendant would leave the
            // subtree with no root — fail loudly before touching any row.
            if uoms.is_self_or_descendant(&mut *tx, uom_id, parent).await? {
                return Err(CatalogWriteError::UomCycle { parent });
            }
        }
        uoms.set_relative(&mut *tx, uom_id, relative_uom_id, relative_factor)
            .await?;
        // Re-derive the stored factors for this unit and its subtree. The function also
        // raises on unreachable rows (cycle/dangling) as the storage-side backstop.
        uoms.recompute_factors(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Convert `qty` from one unit to another over the parent-store tree (ADR-0023).
    ///
    /// Application-side math over the stored factors: `qty * factor_from / factor_to`,
    /// with the rounding policy declared by the caller. Units in different trees fail
    /// LOUDLY with a typed error naming both trees — never a silent numeric result.
    pub async fn convert_quantity(
        &self,
        qty: Decimal,
        from_uom: Uuid,
        to_uom: Uuid,
        rounding: ConversionRounding,
    ) -> Result<Decimal, CatalogWriteError> {
        let uoms = UomRepository::new(self.db_pool.clone());
        let from_rows = uoms
            .load_tree_chain(&self.db_pool, from_uom)
            .await?
            .ok_or(CatalogWriteError::UomNotFound(from_uom))?;
        let to_rows = uoms
            .load_tree_chain(&self.db_pool, to_uom)
            .await?
            .ok_or(CatalogWriteError::UomNotFound(to_uom))?;
        let from = UomChain::from_rows(from_uom, from_rows).map_err(CatalogWriteError::Conversion)?;
        let to = UomChain::from_rows(to_uom, to_rows).map_err(CatalogWriteError::Conversion)?;
        convert_quantity(qty, &from, &to, rounding).map_err(CatalogWriteError::Conversion)
    }

    /// Retire (soft-delete) a unit of measure through the validated path (UM-4).
    ///
    /// User-created units are deletable once nothing leans on them; module-seeded
    /// reference units (`is_protected`) refuse deletion with a typed error — retire
    /// those through the status lifecycle (`active -> inactive`) instead. The guards,
    /// in order: the unit must exist and be live, must not be protected, must not be
    /// the live parent of live units, and must not be the default unit of any live
    /// item (the orphaning hazard that keeps generic delete off the guarded surface,
    /// ADR-005). The database-level protected-unit triggers are the backstop if a
    /// protected row ever slips past the service check.
    pub async fn delete_uom(&self, uom_id: Uuid) -> Result<(), CatalogWriteError> {
        let uoms = UomRepository::new(self.db_pool.clone());
        let mut tx = self.db_pool.begin().await?;
        Self::relay_ambient_scope(&mut tx).await?;

        // All probes run on the transaction connection so the protection state cannot
        // change between the checks and the archive write.
        let code: Option<String> = sqlx::query_scalar(
            "SELECT code FROM catalog.uoms \
             WHERE id = $1 AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(uom_id)
        .fetch_optional(&mut *tx)
        .await?;
        let code = code.ok_or(CatalogWriteError::UomNotFound(uom_id))?;

        if uoms.find_protection(&mut *tx, uom_id).await? != Some(false) {
            return Err(CatalogWriteError::ProtectedUom { code });
        }
        if uoms.count_live_children(&mut *tx, uom_id).await? > 0 {
            return Err(CatalogWriteError::UomHasChildren { code });
        }
        if uoms.exists_live_item_using(&mut *tx, uom_id).await? {
            return Err(CatalogWriteError::UomInUse { code });
        }

        uoms.soft_delete_uom(&mut *tx, uom_id).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_attribute(&self, a: NewAttribute) -> Result<Uuid, CatalogWriteError> {
        let id = Uuid::new_v4();
        let at = a.attribute_type.clone().unwrap_or_else(|| "other".to_string());
        let repo = AttributeRepository::new(self.db_pool.clone());
        let r = repo
            .insert_attribute(
                &self.db_pool,
                &NewAttributeRow {
                    id,
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
    }

    pub async fn create_attribute_value(&self, v: NewAttributeValue) -> Result<Uuid, CatalogWriteError> {
        let attrs = AttributeRepository::new(self.db_pool.clone());
        if !attrs.exists_id(&self.db_pool, v.attribute_id).await? {
            return Err(CatalogWriteError::AttributeNotFound(v.attribute_id));
        }
        let id = Uuid::new_v4();
        let repo = AttributeValueRepository::new(self.db_pool.clone());
        let r = repo
            .insert_attribute_value(
                &self.db_pool,
                &NewAttributeValueRow {
                    id,
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
    }

    /// Create a variant SKU. Validates the item exists, every option maps to a known
    /// attribute+value in the registry, then persists the variant and flips the item's
    /// `has_variants` flag. `variant_label` defaults to the option value labels joined " / ".
    pub async fn create_item_variant(&self, v: NewItemVariant) -> Result<Uuid, CatalogWriteError> {
        let items = ItemRepository::new(self.db_pool.clone());
        if !items.exists_id(&self.db_pool, v.item_id).await? {
            return Err(CatalogWriteError::ItemNotFound(v.item_id));
        }
        if v.options.is_empty() {
            return Err(CatalogWriteError::NoOptions);
        }

        // Validate options against the registry and collect display labels for the label default.
        let attr_values = AttributeValueRepository::new(self.db_pool.clone());
        let attrs = AttributeRepository::new(self.db_pool.clone());
        let mut labels: Vec<String> = Vec::with_capacity(v.options.len());
        for (attr_code, val_code) in &v.options {
            let row = attr_values
                .find_value_with_attribute(&self.db_pool, attr_code, val_code)
                .await?;
            match row {
                Some(r) => labels.push(r.label),
                None => {
                    // Distinguish unknown axis vs unknown value for a clearer error.
                    let attr_ok = attrs.find_id_by_code(&self.db_pool, attr_code).await?;
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
        // Re-bind the ambient org scope (if the composed host set one) so the transaction
        // sees the same fence the pool reads rode.
        Self::relay_ambient_scope(&mut tx).await?;
        let variants = ItemVariantRepository::new(self.db_pool.clone());
        let r = variants
            .insert_variant(
                &mut *tx,
                &NewItemVariantRow {
                    id,
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
    }

    /// Soft-delete a variant and keep `Item.has_variants` honest: if the item has no live variants
    /// left, flip the flag back to false so the storefront picker never lies.
    pub async fn delete_item_variant(&self, variant_id: Uuid) -> Result<(), CatalogWriteError> {
        let variants = ItemVariantRepository::new(self.db_pool.clone());
        let item_id = variants
            .find_item_id_for_live(&self.db_pool, variant_id)
            .await?
            .ok_or(CatalogWriteError::ItemVariantNotFound(variant_id))?;

        let mut tx = self.db_pool.begin().await?;
        Self::relay_ambient_scope(&mut tx).await?;
        variants.soft_delete_variant(&mut *tx, variant_id).await?;
        let remaining = variants.count_live_variants(&mut *tx, item_id).await?;
        if remaining == 0 {
            let items = ItemRepository::new(self.db_pool.clone());
            items.set_has_variants_false(&mut *tx, item_id).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Transition an Item's lifecycle status, enforcing the CatalogStatus state machine declared
    /// in schema/hooks/catalog.hook.yaml (`active ↔ inactive`, `active|inactive → discontinued`;
    /// `discontinued` is terminal).
    pub async fn transition_item_status(
        &self,
        item_id: Uuid,
        target: CatalogStatus,
    ) -> Result<(), CatalogWriteError> {
        let items = ItemRepository::new(self.db_pool.clone());
        let mut tx = self.db_pool.begin().await?;
        Self::relay_ambient_scope(&mut tx).await?;
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

    /// Create a Brand (leaf master). Validated create — same rationale as `create_uom`.
    pub async fn create_brand(&self, b: NewBrand) -> Result<Uuid, CatalogWriteError> {
        let id = Uuid::new_v4();
        let repo = BrandRepository::new(self.db_pool.clone());
        let r = repo
            .insert_brand(
                &self.db_pool,
                &NewBrandRow {
                    id,
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
    }
}
