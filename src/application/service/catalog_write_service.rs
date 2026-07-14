//! Validated write path for Item, ItemGroup, and UomConversion — hand-authored (user-owned).
//!
//! Closes the CRUD-bypass: the generated 12-endpoint CRUD writes rows through `GenericCrudService`
//! with NO domain validation, so a well-formed request could create an Item pointing at a
//! non-existent item group or UOM, an Item that is neither sellable/purchasable/stocked, a
//! self-referential or non-positive UOM conversion, or an item-group whose parent is missing.
//!
//! `CatalogModule` mounts these validated writers via `create_guarded_catalog_routes`.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

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
    DuplicateUomCode(String),
    DuplicateBrandCode(String),
    DuplicateAttributeCode(String),
    DuplicateValueCode(String),
    DuplicateSku(String),
    NoOptions,
    UnknownAttribute(String),
    UnknownAttributeValue(String),
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
            CatalogWriteError::DuplicateUomCode(_) => "duplicate_uom_code",
            CatalogWriteError::DuplicateBrandCode(_) => "duplicate_brand_code",
            CatalogWriteError::DuplicateAttributeCode(_) => "duplicate_attribute_code",
            CatalogWriteError::DuplicateValueCode(_) => "duplicate_value_code",
            CatalogWriteError::DuplicateSku(_) => "duplicate_sku",
            CatalogWriteError::NoOptions => "no_options",
            CatalogWriteError::UnknownAttribute(_) => "unknown_attribute",
            CatalogWriteError::UnknownAttributeValue(_) => "unknown_attribute_value",
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
    pub tags: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}

/// Physical (stockable-capable) item types. Non-physical types are never stockable.
pub fn is_physical_item_type(item_type: &str) -> bool {
    matches!(item_type, "physical_good" | "bundle" | "rental")
}

#[derive(Debug, Clone)]
pub struct NewUomConversion {
    pub from_uom_id: Uuid,
    pub to_uom_id: Uuid,
    pub factor: Decimal,
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

#[derive(Debug, Clone)]
pub struct NewUom {
    pub code: String,
    pub name: String,
    pub uom_type: Option<String>,
    pub decimal_places: i32,
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

/// A scan resolved to a sellable identity: the item (always) plus the variant if the scanned code
/// matched a variant SKU/barcode rather than the base item. POS rings against `item_id`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ItemHit {
    pub item_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub item_code: String,
    pub name: String,
    pub barcode: Option<String>,
    pub sku: Option<String>,
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
    /// code. Read-only; the codes are DB-unique so at most one row matches.
    pub async fn lookup_item(&self, code: &str) -> Result<Option<ItemHit>, CatalogWriteError> {
        if let Some(hit) = sqlx::query_as::<_, ItemHit>(
            r#"SELECT id AS item_id, NULL::uuid AS variant_id, item_code, name, barcode, NULL::text AS sku
               FROM catalog.items
               WHERE (barcode = $1 OR item_code = $1) AND (metadata->>'deleted_at') IS NULL
               LIMIT 1"#,
        ).bind(code).fetch_optional(&self.db_pool).await? {
            return Ok(Some(hit));
        }
        let hit = sqlx::query_as::<_, ItemHit>(
            r#"SELECT v.item_id, v.id AS variant_id, i.item_code, i.name, v.barcode, v.sku
               FROM catalog.item_variants v JOIN catalog.items i ON i.id = v.item_id
               WHERE (v.barcode = $1 OR v.sku = $1) AND (v.metadata->>'deleted_at') IS NULL
               LIMIT 1"#,
        ).bind(code).fetch_optional(&self.db_pool).await?;
        Ok(hit)
    }

    async fn exists(&self, table: &str, id: Uuid) -> Result<bool, CatalogWriteError> {
        // `table` is a fixed literal from this module, never user input.
        let sql = format!(
            "SELECT id FROM catalog.{table} WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"
        );
        let found: Option<Uuid> = sqlx::query_scalar(&sql)
            .bind(id)
            .fetch_optional(&self.db_pool)
            .await?;
        Ok(found.is_some())
    }

    fn is_dup(e: &sqlx::Error, needle: &str) -> bool {
        e.as_database_error()
            .map(|d| d.is_unique_violation() && d.constraint().unwrap_or("").contains(needle))
            .unwrap_or(false)
    }

    pub async fn create_item_group(&self, g: NewItemGroup) -> Result<Uuid, CatalogWriteError> {
        if let Some(pid) = g.parent_id {
            if !self.exists("item_groups", pid).await? {
                return Err(CatalogWriteError::ParentNotFound(pid));
            }
        }
        let id = Uuid::new_v4();
        let r = sqlx::query(
            r#"INSERT INTO catalog.item_groups (id, code, name, parent_id, is_group, status)
               VALUES ($1,$2,$3,$4,$5,'active'::catalog_status)"#,
        )
        .bind(id)
        .bind(&g.code)
        .bind(&g.name)
        .bind(g.parent_id)
        .bind(g.is_group)
        .execute(&self.db_pool)
        .await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if Self::is_dup(&e, "code") => {
                Err(CatalogWriteError::DuplicateItemCode(g.code))
            }
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
        if !self.exists("item_groups", i.item_group_id).await? {
            return Err(CatalogWriteError::ItemGroupNotFound(i.item_group_id));
        }
        if !self.exists("uoms", i.default_uom_id).await? {
            return Err(CatalogWriteError::UomNotFound(i.default_uom_id));
        }
        if let Some(bid) = i.brand_id {
            if !self.exists("brands", bid).await? {
                return Err(CatalogWriteError::BrandNotFound(bid));
            }
        }
        let id = Uuid::new_v4();
        let tags = i.tags.clone().unwrap_or_else(|| serde_json::json!([]));
        let data = i.data.clone().unwrap_or_else(|| serde_json::json!({}));
        let r = sqlx::query(
            r#"INSERT INTO catalog.items
                (id, item_code, name, description, barcode, brand_id, item_group_id,
                 default_uom_id, item_type, is_sales_item, is_purchase_item, is_stock_item,
                 hsn_code, is_taxable, weight_per_unit, tags, data, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::item_type,$10,$11,$12,$13,$14,$15,$16,$17,'active'::catalog_status)"#,
        )
        .bind(id)
        .bind(&i.item_code)
        .bind(&i.name)
        .bind(&i.description)
        .bind(&i.barcode)
        .bind(i.brand_id)
        .bind(i.item_group_id)
        .bind(i.default_uom_id)
        .bind(&item_type)
        .bind(i.is_sales_item)
        .bind(i.is_purchase_item)
        .bind(is_stock_item)
        .bind(&i.hsn_code)
        .bind(i.is_taxable)
        .bind(i.weight_per_unit)
        .bind(&tags)
        .bind(&data)
        .execute(&self.db_pool)
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

    pub async fn create_uom_conversion(
        &self,
        c: NewUomConversion,
    ) -> Result<Uuid, CatalogWriteError> {
        if c.from_uom_id == c.to_uom_id {
            return Err(CatalogWriteError::SameUom);
        }
        if c.factor <= Decimal::ZERO {
            return Err(CatalogWriteError::NonPositiveFactor);
        }
        if !self.exists("uoms", c.from_uom_id).await? {
            return Err(CatalogWriteError::UomNotFound(c.from_uom_id));
        }
        if !self.exists("uoms", c.to_uom_id).await? {
            return Err(CatalogWriteError::UomNotFound(c.to_uom_id));
        }
        let id = Uuid::new_v4();
        let r = sqlx::query(
            r#"INSERT INTO catalog.uom_conversions (id, from_uom_id, to_uom_id, factor)
               VALUES ($1,$2,$3,$4)"#,
        )
        .bind(id)
        .bind(c.from_uom_id)
        .bind(c.to_uom_id)
        .bind(c.factor)
        .execute(&self.db_pool)
        .await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) => {
                Err(CatalogWriteError::DuplicateConversion)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_attribute(&self, a: NewAttribute) -> Result<Uuid, CatalogWriteError> {
        let id = Uuid::new_v4();
        let at = a.attribute_type.clone().unwrap_or_else(|| "other".to_string());
        let r = sqlx::query(
            r#"INSERT INTO catalog.attributes (id, code, name, attribute_type, status)
               VALUES ($1,$2,$3,$4::attribute_type,'active'::catalog_status)"#,
        )
        .bind(id).bind(&a.code).bind(&a.name).bind(&at)
        .execute(&self.db_pool).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateAttributeCode(a.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_attribute_value(&self, v: NewAttributeValue) -> Result<Uuid, CatalogWriteError> {
        if !self.exists("attributes", v.attribute_id).await? {
            return Err(CatalogWriteError::AttributeNotFound(v.attribute_id));
        }
        let id = Uuid::new_v4();
        let r = sqlx::query(
            r#"INSERT INTO catalog.attribute_values
                (id, attribute_id, code, label, label_en, swatch_hex, sort_order, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,'active'::catalog_status)"#,
        )
        .bind(id).bind(v.attribute_id).bind(&v.code).bind(&v.label)
        .bind(&v.label_en).bind(&v.swatch_hex).bind(v.sort_order)
        .execute(&self.db_pool).await;
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
        if !self.exists("items", v.item_id).await? {
            return Err(CatalogWriteError::ItemNotFound(v.item_id));
        }
        if v.options.is_empty() {
            return Err(CatalogWriteError::NoOptions);
        }

        // Validate options against the registry and collect display labels for the label default.
        let mut labels: Vec<String> = Vec::with_capacity(v.options.len());
        for (attr_code, val_code) in &v.options {
            let row: Option<(Uuid, String)> = sqlx::query_as(
                r#"SELECT av.id, av.label
                   FROM catalog.attribute_values av
                   JOIN catalog.attributes a ON a.id = av.attribute_id
                   WHERE a.code = $1 AND av.code = $2
                     AND (a.metadata->>'deleted_at') IS NULL
                     AND (av.metadata->>'deleted_at') IS NULL"#,
            )
            .bind(attr_code)
            .bind(val_code)
            .fetch_optional(&self.db_pool)
            .await?;
            match row {
                Some((_, label)) => labels.push(label),
                None => {
                    // Distinguish unknown axis vs unknown value for a clearer error.
                    let attr_ok: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM catalog.attributes WHERE code=$1 AND (metadata->>'deleted_at') IS NULL",
                    )
                    .bind(attr_code).fetch_optional(&self.db_pool).await?;
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
        let r = sqlx::query(
            r#"INSERT INTO catalog.item_variants
                (id, item_id, sku, variant_label, options, barcode, is_default, weight_per_unit, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'active'::catalog_status)"#,
        )
        .bind(id).bind(v.item_id).bind(&v.sku).bind(&label).bind(&options_json)
        .bind(&v.barcode).bind(v.is_default).bind(v.weight_per_unit)
        .execute(&mut *tx).await;
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
        sqlx::query("UPDATE catalog.items SET has_variants = TRUE WHERE id = $1")
            .bind(v.item_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Soft-delete a variant and keep `Item.has_variants` honest: if the item has no live variants
    /// left, flip the flag back to false so the storefront picker never lies.
    pub async fn delete_item_variant(&self, variant_id: Uuid) -> Result<(), CatalogWriteError> {
        let item_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT item_id FROM catalog.item_variants WHERE id=$1 AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(variant_id)
        .fetch_optional(&self.db_pool)
        .await?;
        let item_id = item_id.ok_or(CatalogWriteError::ItemVariantNotFound(variant_id))?;

        let mut tx = self.db_pool.begin().await?;
        sqlx::query(
            "UPDATE catalog.item_variants SET metadata = jsonb_set(metadata, '{deleted_at}', to_jsonb(now())) WHERE id=$1",
        )
        .bind(variant_id)
        .execute(&mut *tx)
        .await?;
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM catalog.item_variants WHERE item_id=$1 AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
        if remaining == 0 {
            sqlx::query("UPDATE catalog.items SET has_variants = FALSE WHERE id=$1")
                .bind(item_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Create a Uom (leaf master). Validated create so the guarded surface can mount Uom read-only
    /// (generic delete/patch would orphan items that FK-point at it — council 2026-07-01).
    pub async fn create_uom(&self, u: NewUom) -> Result<Uuid, CatalogWriteError> {
        let id = Uuid::new_v4();
        let ut = u.uom_type.clone().unwrap_or_else(|| "count".to_string());
        let r = sqlx::query(
            r#"INSERT INTO catalog.uoms (id, code, name, uom_type, decimal_places, status)
               VALUES ($1,$2,$3,$4::uom_type,$5,'active'::catalog_status)"#,
        )
        .bind(id).bind(&u.code).bind(&u.name).bind(&ut).bind(u.decimal_places)
        .execute(&self.db_pool).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateUomCode(u.code)),
            Err(e) => Err(e.into()),
        }
    }

    /// Create a Brand (leaf master). Validated create — same rationale as `create_uom`.
    pub async fn create_brand(&self, b: NewBrand) -> Result<Uuid, CatalogWriteError> {
        let id = Uuid::new_v4();
        let r = sqlx::query(
            r#"INSERT INTO catalog.brands
                (id, code, name, short_description, description, logo_url, sort_order, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,'active'::catalog_status)"#,
        )
        .bind(id).bind(&b.code).bind(&b.name).bind(&b.short_description)
        .bind(&b.description).bind(&b.logo_url).bind(b.sort_order)
        .execute(&self.db_pool).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if Self::is_dup(&e, "code") => Err(CatalogWriteError::DuplicateBrandCode(b.code)),
            Err(e) => Err(e.into()),
        }
    }
}
