use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ItemType;
use super::CatalogStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub Uuid);

impl ItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ItemId> for Uuid {
    fn from(id: ItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for ItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Item {
    pub id: Uuid,
    pub item_code: String,
    pub name: String,
    pub description: Option<String>,
    pub barcode: Option<String>,
    pub brand_id: Option<Uuid>,
    pub item_group_id: Uuid,
    pub default_uom_id: Uuid,
    pub item_type: ItemType,
    pub is_sales_item: bool,
    pub is_purchase_item: bool,
    pub is_stock_item: bool,
    pub has_variants: bool,
    pub hsn_code: Option<String>,
    pub sni: Option<String>,
    pub is_taxable: bool,
    pub weight_per_unit: Option<Decimal>,
    pub shelf_life_days: Option<i32>,
    pub tags: serde_json::Value,
    pub data: serde_json::Value,
    pub status: CatalogStatus,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Item {
    /// Create a builder for Item
    pub fn builder() -> ItemBuilder {
        ItemBuilder::default()
    }

    /// Create a new Item with required fields
    pub fn new(item_code: String, name: String, item_group_id: Uuid, default_uom_id: Uuid, item_type: ItemType, is_sales_item: bool, is_purchase_item: bool, is_stock_item: bool, has_variants: bool, is_taxable: bool, tags: serde_json::Value, data: serde_json::Value, status: CatalogStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            item_code,
            name,
            description: None,
            barcode: None,
            brand_id: None,
            item_group_id,
            default_uom_id,
            item_type,
            is_sales_item,
            is_purchase_item,
            is_stock_item,
            has_variants,
            hsn_code: None,
            sni: None,
            is_taxable,
            weight_per_unit: None,
            shelf_life_days: None,
            tags,
            data,
            status,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ItemId {
        ItemId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }

    /// Get the current status
    pub fn status(&self) -> &CatalogStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the barcode field (chainable)
    pub fn with_barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the brand_id field (chainable)
    pub fn with_brand_id(mut self, value: Uuid) -> Self {
        self.brand_id = Some(value);
        self
    }

    /// Set the hsn_code field (chainable)
    pub fn with_hsn_code(mut self, value: String) -> Self {
        self.hsn_code = Some(value);
        self
    }

    /// Set the sni field (chainable)
    pub fn with_sni(mut self, value: String) -> Self {
        self.sni = Some(value);
        self
    }

    /// Set the weight_per_unit field (chainable)
    pub fn with_weight_per_unit(mut self, value: Decimal) -> Self {
        self.weight_per_unit = Some(value);
        self
    }

    /// Set the shelf_life_days field (chainable)
    pub fn with_shelf_life_days(mut self, value: i32) -> Self {
        self.shelf_life_days = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "item_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                "brand_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.brand_id = v; }
                }
                "item_group_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_group_id = v; }
                }
                "default_uom_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_uom_id = v; }
                }
                "item_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_type = v; }
                }
                "is_sales_item" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_sales_item = v; }
                }
                "is_purchase_item" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_purchase_item = v; }
                }
                "is_stock_item" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_stock_item = v; }
                }
                "has_variants" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_variants = v; }
                }
                "hsn_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.hsn_code = v; }
                }
                "sni" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sni = v; }
                }
                "is_taxable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_taxable = v; }
                }
                "weight_per_unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.weight_per_unit = v; }
                }
                "shelf_life_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shelf_life_days = v; }
                }
                "tags" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tags = v; }
                }
                "data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.data = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Item {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Item"
    }
}

impl backbone_core::PersistentEntity for Item {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Item {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("brand_id".to_string(), "uuid".to_string());
        m.insert("item_group_id".to_string(), "uuid".to_string());
        m.insert("default_uom_id".to_string(), "uuid".to_string());
        m.insert("item_type".to_string(), "item_type".to_string());
        m.insert("status".to_string(), "catalog_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["item_code", "name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("itemGroup", "item_groups", "itemGroupId"), ("brand", "brands", "brandId"), ("defaultUom", "uoms", "defaultUomId")]
    }
}

/// Builder for Item entity
///
/// Provides a fluent API for constructing Item instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ItemBuilder {
    item_code: Option<String>,
    name: Option<String>,
    description: Option<String>,
    barcode: Option<String>,
    brand_id: Option<Uuid>,
    item_group_id: Option<Uuid>,
    default_uom_id: Option<Uuid>,
    item_type: Option<ItemType>,
    is_sales_item: Option<bool>,
    is_purchase_item: Option<bool>,
    is_stock_item: Option<bool>,
    has_variants: Option<bool>,
    hsn_code: Option<String>,
    sni: Option<String>,
    is_taxable: Option<bool>,
    weight_per_unit: Option<Decimal>,
    shelf_life_days: Option<i32>,
    tags: Option<serde_json::Value>,
    data: Option<serde_json::Value>,
    status: Option<CatalogStatus>,
}

impl ItemBuilder {
    /// Set the item_code field (required)
    pub fn item_code(mut self, value: String) -> Self {
        self.item_code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the barcode field (optional)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the brand_id field (optional)
    pub fn brand_id(mut self, value: Uuid) -> Self {
        self.brand_id = Some(value);
        self
    }

    /// Set the item_group_id field (required)
    pub fn item_group_id(mut self, value: Uuid) -> Self {
        self.item_group_id = Some(value);
        self
    }

    /// Set the default_uom_id field (required)
    pub fn default_uom_id(mut self, value: Uuid) -> Self {
        self.default_uom_id = Some(value);
        self
    }

    /// Set the item_type field (default: `ItemType::default()`)
    pub fn item_type(mut self, value: ItemType) -> Self {
        self.item_type = Some(value);
        self
    }

    /// Set the is_sales_item field (default: `true`)
    pub fn is_sales_item(mut self, value: bool) -> Self {
        self.is_sales_item = Some(value);
        self
    }

    /// Set the is_purchase_item field (default: `true`)
    pub fn is_purchase_item(mut self, value: bool) -> Self {
        self.is_purchase_item = Some(value);
        self
    }

    /// Set the is_stock_item field (default: `true`)
    pub fn is_stock_item(mut self, value: bool) -> Self {
        self.is_stock_item = Some(value);
        self
    }

    /// Set the has_variants field (default: `false`)
    pub fn has_variants(mut self, value: bool) -> Self {
        self.has_variants = Some(value);
        self
    }

    /// Set the hsn_code field (optional)
    pub fn hsn_code(mut self, value: String) -> Self {
        self.hsn_code = Some(value);
        self
    }

    /// Set the sni field (optional)
    pub fn sni(mut self, value: String) -> Self {
        self.sni = Some(value);
        self
    }

    /// Set the is_taxable field (default: `true`)
    pub fn is_taxable(mut self, value: bool) -> Self {
        self.is_taxable = Some(value);
        self
    }

    /// Set the weight_per_unit field (optional)
    pub fn weight_per_unit(mut self, value: Decimal) -> Self {
        self.weight_per_unit = Some(value);
        self
    }

    /// Set the shelf_life_days field (optional)
    pub fn shelf_life_days(mut self, value: i32) -> Self {
        self.shelf_life_days = Some(value);
        self
    }

    /// Set the tags field (default: `serde_json::json!([])`)
    pub fn tags(mut self, value: serde_json::Value) -> Self {
        self.tags = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Set the status field (default: `CatalogStatus::default()`)
    pub fn status(mut self, value: CatalogStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Build the Item entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Item, String> {
        let item_code = self.item_code.ok_or_else(|| "item_code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let item_group_id = self.item_group_id.ok_or_else(|| "item_group_id is required".to_string())?;
        let default_uom_id = self.default_uom_id.ok_or_else(|| "default_uom_id is required".to_string())?;

        Ok(Item {
            id: Uuid::new_v4(),
            item_code,
            name,
            description: self.description,
            barcode: self.barcode,
            brand_id: self.brand_id,
            item_group_id,
            default_uom_id,
            item_type: self.item_type.unwrap_or(ItemType::default()),
            is_sales_item: self.is_sales_item.unwrap_or(true),
            is_purchase_item: self.is_purchase_item.unwrap_or(true),
            is_stock_item: self.is_stock_item.unwrap_or(true),
            has_variants: self.has_variants.unwrap_or(false),
            hsn_code: self.hsn_code,
            sni: self.sni,
            is_taxable: self.is_taxable.unwrap_or(true),
            weight_per_unit: self.weight_per_unit,
            shelf_life_days: self.shelf_life_days,
            tags: self.tags.unwrap_or(serde_json::json!([])),
            data: self.data.unwrap_or(serde_json::json!({})),
            status: self.status.unwrap_or(CatalogStatus::default()),
            metadata: AuditMetadata::default(),
        })
    }
}
