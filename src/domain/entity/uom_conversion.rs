use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for UomConversion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UomConversionId(pub Uuid);

impl UomConversionId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for UomConversionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UomConversionId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for UomConversionId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<UomConversionId> for Uuid {
    fn from(id: UomConversionId) -> Self { id.0 }
}

impl AsRef<Uuid> for UomConversionId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for UomConversionId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UomConversion {
    pub id: Uuid,
    pub company_id: Uuid,
    pub from_uom_id: Uuid,
    pub to_uom_id: Uuid,
    pub factor: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl UomConversion {
    /// Create a builder for UomConversion
    pub fn builder() -> UomConversionBuilder {
        UomConversionBuilder::default()
    }

    /// Create a new UomConversion with required fields
    pub fn new(company_id: Uuid, from_uom_id: Uuid, to_uom_id: Uuid, factor: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            from_uom_id,
            to_uom_id,
            factor,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> UomConversionId {
        UomConversionId(self.id)
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
                "from_uom_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_uom_id = v; }
                }
                "to_uom_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_uom_id = v; }
                }
                "factor" => {
                    if let Ok(v) = serde_json::from_value(value) { self.factor = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for UomConversion {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "UomConversion"
    }
}

impl backbone_core::PersistentEntity for UomConversion {
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

impl backbone_orm::EntityRepoMeta for UomConversion {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("from_uom_id".to_string(), "uuid".to_string());
        m.insert("to_uom_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("fromUom", "uoms", "fromUomId"), ("toUom", "uoms", "toUomId")]
    }
}

/// Builder for UomConversion entity
///
/// Provides a fluent API for constructing UomConversion instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct UomConversionBuilder {
    company_id: Option<Uuid>,
    from_uom_id: Option<Uuid>,
    to_uom_id: Option<Uuid>,
    factor: Option<Decimal>,
}

impl UomConversionBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the from_uom_id field (required)
    pub fn from_uom_id(mut self, value: Uuid) -> Self {
        self.from_uom_id = Some(value);
        self
    }

    /// Set the to_uom_id field (required)
    pub fn to_uom_id(mut self, value: Uuid) -> Self {
        self.to_uom_id = Some(value);
        self
    }

    /// Set the factor field (required)
    pub fn factor(mut self, value: Decimal) -> Self {
        self.factor = Some(value);
        self
    }

    /// Build the UomConversion entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<UomConversion, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let from_uom_id = self.from_uom_id.ok_or_else(|| "from_uom_id is required".to_string())?;
        let to_uom_id = self.to_uom_id.ok_or_else(|| "to_uom_id is required".to_string())?;
        let factor = self.factor.ok_or_else(|| "factor is required".to_string())?;

        Ok(UomConversion {
            id: Uuid::new_v4(),
            company_id,
            from_uom_id,
            to_uom_id,
            factor,
            metadata: AuditMetadata::default(),
        })
    }
}
