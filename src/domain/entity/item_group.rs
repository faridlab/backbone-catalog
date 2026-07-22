use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::CatalogStatus;
use super::AuditMetadata;

/// Strongly-typed ID for ItemGroup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemGroupId(pub Uuid);

impl ItemGroupId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ItemGroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ItemGroupId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ItemGroupId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ItemGroupId> for Uuid {
    fn from(id: ItemGroupId) -> Self { id.0 }
}

impl AsRef<Uuid> for ItemGroupId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ItemGroupId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ItemGroup {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub is_group: bool,
    pub level: i32,
    pub sort_order: i32,
    pub status: CatalogStatus,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ItemGroup {
    /// Create a builder for ItemGroup
    pub fn builder() -> ItemGroupBuilder {
        ItemGroupBuilder::default()
    }

    /// Create a new ItemGroup with required fields
    pub fn new(company_id: Uuid, code: String, name: String, is_group: bool, level: i32, sort_order: i32, status: CatalogStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            parent_id: None,
            is_group,
            level,
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
    pub fn typed_id(&self) -> ItemGroupId {
        ItemGroupId(self.id)
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

    /// Set the parent_id field (chainable)
    pub fn with_parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
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
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "parent_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_id = v; }
                }
                "is_group" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_group = v; }
                }
                "level" => {
                    if let Ok(v) = serde_json::from_value(value) { self.level = v; }
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

impl super::Entity for ItemGroup {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ItemGroup"
    }
}

impl backbone_core::PersistentEntity for ItemGroup {
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

impl backbone_orm::EntityRepoMeta for ItemGroup {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("parent_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "catalog_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("parent", "item_groups", "parentId")]
    }
}

/// Builder for ItemGroup entity
///
/// Provides a fluent API for constructing ItemGroup instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ItemGroupBuilder {
    company_id: Option<Uuid>,
    code: Option<String>,
    name: Option<String>,
    parent_id: Option<Uuid>,
    is_group: Option<bool>,
    level: Option<i32>,
    sort_order: Option<i32>,
    status: Option<CatalogStatus>,
}

impl ItemGroupBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the code field (required)
    pub fn code(mut self, value: String) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the parent_id field (optional)
    pub fn parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the is_group field (default: `false`)
    pub fn is_group(mut self, value: bool) -> Self {
        self.is_group = Some(value);
        self
    }

    /// Set the level field (default: `0`)
    pub fn level(mut self, value: i32) -> Self {
        self.level = Some(value);
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

    /// Build the ItemGroup entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ItemGroup, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(ItemGroup {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            parent_id: self.parent_id,
            is_group: self.is_group.unwrap_or(false),
            level: self.level.unwrap_or(0),
            sort_order: self.sort_order.unwrap_or(0),
            status: self.status.unwrap_or(CatalogStatus::default()),
            metadata: AuditMetadata::default(),
        })
    }
}
