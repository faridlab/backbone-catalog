use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::CatalogStatus;
use super::AuditMetadata;

/// Strongly-typed ID for AttributeValue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeValueId(pub Uuid);

impl AttributeValueId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AttributeValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AttributeValueId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AttributeValueId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AttributeValueId> for Uuid {
    fn from(id: AttributeValueId) -> Self { id.0 }
}

impl AsRef<Uuid> for AttributeValueId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AttributeValueId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AttributeValue {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
    pub label: String,
    pub label_en: Option<String>,
    pub swatch_hex: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub status: CatalogStatus,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl AttributeValue {
    /// Create a builder for AttributeValue
    pub fn builder() -> AttributeValueBuilder {
        AttributeValueBuilder::default()
    }

    /// Create a new AttributeValue with required fields
    pub fn new(company_id: Uuid, attribute_id: Uuid, code: String, label: String, sort_order: i32, status: CatalogStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            attribute_id,
            code,
            label,
            label_en: None,
            swatch_hex: None,
            icon: None,
            sort_order,
            status,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AttributeValueId {
        AttributeValueId(self.id)
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

    /// Set the label_en field (chainable)
    pub fn with_label_en(mut self, value: String) -> Self {
        self.label_en = Some(value);
        self
    }

    /// Set the swatch_hex field (chainable)
    pub fn with_swatch_hex(mut self, value: String) -> Self {
        self.swatch_hex = Some(value);
        self
    }

    /// Set the icon field (chainable)
    pub fn with_icon(mut self, value: String) -> Self {
        self.icon = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "attribute_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.attribute_id = v; }
                }
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "label" => {
                    if let Ok(v) = serde_json::from_value(value) { self.label = v; }
                }
                "label_en" => {
                    if let Ok(v) = serde_json::from_value(value) { self.label_en = v; }
                }
                "swatch_hex" => {
                    if let Ok(v) = serde_json::from_value(value) { self.swatch_hex = v; }
                }
                "icon" => {
                    if let Ok(v) = serde_json::from_value(value) { self.icon = v; }
                }
                "sort_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sort_order = v; }
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

impl super::Entity for AttributeValue {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "AttributeValue"
    }
}

impl backbone_core::PersistentEntity for AttributeValue {
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

impl backbone_orm::EntityRepoMeta for AttributeValue {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("attribute_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "catalog_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "label"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("attribute", "attributes", "attributeId")]
    }
}

/// Builder for AttributeValue entity
///
/// Provides a fluent API for constructing AttributeValue instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AttributeValueBuilder {
    company_id: Option<Uuid>,
    attribute_id: Option<Uuid>,
    code: Option<String>,
    label: Option<String>,
    label_en: Option<String>,
    swatch_hex: Option<String>,
    icon: Option<String>,
    sort_order: Option<i32>,
    status: Option<CatalogStatus>,
}

impl AttributeValueBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the attribute_id field (required)
    pub fn attribute_id(mut self, value: Uuid) -> Self {
        self.attribute_id = Some(value);
        self
    }

    /// Set the code field (required)
    pub fn code(mut self, value: String) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the label field (required)
    pub fn label(mut self, value: String) -> Self {
        self.label = Some(value);
        self
    }

    /// Set the label_en field (optional)
    pub fn label_en(mut self, value: String) -> Self {
        self.label_en = Some(value);
        self
    }

    /// Set the swatch_hex field (optional)
    pub fn swatch_hex(mut self, value: String) -> Self {
        self.swatch_hex = Some(value);
        self
    }

    /// Set the icon field (optional)
    pub fn icon(mut self, value: String) -> Self {
        self.icon = Some(value);
        self
    }

    /// Set the sort_order field (default: `0`)
    pub fn sort_order(mut self, value: i32) -> Self {
        self.sort_order = Some(value);
        self
    }

    /// Set the status field (default: `CatalogStatus::default()`)
    pub fn status(mut self, value: CatalogStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Build the AttributeValue entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<AttributeValue, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let attribute_id = self.attribute_id.ok_or_else(|| "attribute_id is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let label = self.label.ok_or_else(|| "label is required".to_string())?;

        Ok(AttributeValue {
            id: Uuid::new_v4(),
            company_id,
            attribute_id,
            code,
            label,
            label_en: self.label_en,
            swatch_hex: self.swatch_hex,
            icon: self.icon,
            sort_order: self.sort_order.unwrap_or(0),
            status: self.status.unwrap_or(CatalogStatus::default()),
            metadata: AuditMetadata::default(),
        })
    }
}
