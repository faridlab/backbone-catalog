use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "item_type", rename_all = "snake_case")]
pub enum ItemType {
    PhysicalGood,
    DigitalGood,
    Service,
    Subscription,
    Bundle,
    GiftCard,
    Rental,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhysicalGood => write!(f, "physical_good"),
            Self::DigitalGood => write!(f, "digital_good"),
            Self::Service => write!(f, "service"),
            Self::Subscription => write!(f, "subscription"),
            Self::Bundle => write!(f, "bundle"),
            Self::GiftCard => write!(f, "gift_card"),
            Self::Rental => write!(f, "rental"),
        }
    }
}

impl FromStr for ItemType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "physical_good" => Ok(Self::PhysicalGood),
            "digital_good" => Ok(Self::DigitalGood),
            "service" => Ok(Self::Service),
            "subscription" => Ok(Self::Subscription),
            "bundle" => Ok(Self::Bundle),
            "gift_card" => Ok(Self::GiftCard),
            "rental" => Ok(Self::Rental),
            _ => Err(format!("Unknown ItemType variant: {}", s)),
        }
    }
}

impl Default for ItemType {
    fn default() -> Self {
        Self::PhysicalGood
    }
}
