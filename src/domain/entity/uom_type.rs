use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "uom_type", rename_all = "snake_case")]
pub enum UomType {
    Count,
    Weight,
    Volume,
    Length,
    Area,
    Time,
    Other,
}

impl std::fmt::Display for UomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Count => write!(f, "count"),
            Self::Weight => write!(f, "weight"),
            Self::Volume => write!(f, "volume"),
            Self::Length => write!(f, "length"),
            Self::Area => write!(f, "area"),
            Self::Time => write!(f, "time"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl FromStr for UomType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "count" => Ok(Self::Count),
            "weight" => Ok(Self::Weight),
            "volume" => Ok(Self::Volume),
            "length" => Ok(Self::Length),
            "area" => Ok(Self::Area),
            "time" => Ok(Self::Time),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown UomType variant: {}", s)),
        }
    }
}

impl Default for UomType {
    fn default() -> Self {
        Self::Count
    }
}
