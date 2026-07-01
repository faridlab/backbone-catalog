use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::CatalogStatus;
use super::AuditMetadata;

/// Strongly-typed ID for ItemVariant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemVariantId(pub Uuid);

impl ItemVariantId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ItemVariantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ItemVariantId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ItemVariantId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ItemVariantId> for Uuid {
    fn from(id: ItemVariantId) -> Self { id.0 }
}

impl AsRef<Uuid> for ItemVariantId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ItemVariantId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ItemVariant {
    pub id: Uuid,
    pub item_id: Uuid,
    pub sku: String,
    pub variant_label: String,
    pub options: serde_json::Value,
    pub barcode: Option<String>,
    pub is_default: bool,
    pub weight_per_unit: Option<Decimal>,
    pub status: CatalogStatus,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ItemVariant {
    /// Create a builder for ItemVariant
    pub fn builder() -> ItemVariantBuilder {
        ItemVariantBuilder::default()
    }

    /// Create a new ItemVariant with required fields
    pub fn new(item_id: Uuid, sku: String, variant_label: String, options: serde_json::Value, is_default: bool, status: CatalogStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            item_id,
            sku,
            variant_label,
            options,
            barcode: None,
            is_default,
            weight_per_unit: None,
            status,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ItemVariantId {
        ItemVariantId(self.id)
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

    /// Set the barcode field (chainable)
    pub fn with_barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the weight_per_unit field (chainable)
    pub fn with_weight_per_unit(mut self, value: Decimal) -> Self {
        self.weight_per_unit = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "sku" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sku = v; }
                }
                "variant_label" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variant_label = v; }
                }
                "options" => {
                    if let Ok(v) = serde_json::from_value(value) { self.options = v; }
                }
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                "is_default" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_default = v; }
                }
                "weight_per_unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.weight_per_unit = v; }
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

impl super::Entity for ItemVariant {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ItemVariant"
    }
}

impl backbone_core::PersistentEntity for ItemVariant {
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

impl backbone_orm::EntityRepoMeta for ItemVariant {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "catalog_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["sku", "variant_label"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("item", "items", "itemId")]
    }
}

/// Builder for ItemVariant entity
///
/// Provides a fluent API for constructing ItemVariant instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ItemVariantBuilder {
    item_id: Option<Uuid>,
    sku: Option<String>,
    variant_label: Option<String>,
    options: Option<serde_json::Value>,
    barcode: Option<String>,
    is_default: Option<bool>,
    weight_per_unit: Option<Decimal>,
    status: Option<CatalogStatus>,
}

impl ItemVariantBuilder {
    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the sku field (required)
    pub fn sku(mut self, value: String) -> Self {
        self.sku = Some(value);
        self
    }

    /// Set the variant_label field (required)
    pub fn variant_label(mut self, value: String) -> Self {
        self.variant_label = Some(value);
        self
    }

    /// Set the options field (default: `serde_json::json!({})`)
    pub fn options(mut self, value: serde_json::Value) -> Self {
        self.options = Some(value);
        self
    }

    /// Set the barcode field (optional)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the is_default field (default: `false`)
    pub fn is_default(mut self, value: bool) -> Self {
        self.is_default = Some(value);
        self
    }

    /// Set the weight_per_unit field (optional)
    pub fn weight_per_unit(mut self, value: Decimal) -> Self {
        self.weight_per_unit = Some(value);
        self
    }

    /// Set the status field (default: `CatalogStatus::default()`)
    pub fn status(mut self, value: CatalogStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Build the ItemVariant entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ItemVariant, String> {
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let sku = self.sku.ok_or_else(|| "sku is required".to_string())?;
        let variant_label = self.variant_label.ok_or_else(|| "variant_label is required".to_string())?;

        Ok(ItemVariant {
            id: Uuid::new_v4(),
            item_id,
            sku,
            variant_label,
            options: self.options.unwrap_or(serde_json::json!({})),
            barcode: self.barcode,
            is_default: self.is_default.unwrap_or(false),
            weight_per_unit: self.weight_per_unit,
            status: self.status.unwrap_or(CatalogStatus::default()),
            metadata: AuditMetadata::default(),
        })
    }
}
