use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "attribute_type", rename_all = "snake_case")]
pub enum AttributeType {
    Color,
    Size,
    Material,
    Style,
    Capacity,
    Other,
}

impl std::fmt::Display for AttributeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Color => write!(f, "color"),
            Self::Size => write!(f, "size"),
            Self::Material => write!(f, "material"),
            Self::Style => write!(f, "style"),
            Self::Capacity => write!(f, "capacity"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl FromStr for AttributeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "color" => Ok(Self::Color),
            "size" => Ok(Self::Size),
            "material" => Ok(Self::Material),
            "style" => Ok(Self::Style),
            "capacity" => Ok(Self::Capacity),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown AttributeType variant: {}", s)),
        }
    }
}

impl Default for AttributeType {
    fn default() -> Self {
        Self::Other
    }
}
