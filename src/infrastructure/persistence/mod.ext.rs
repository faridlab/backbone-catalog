// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The hand-written catalog SQL's parameter/projection types. Every repository listed here is
// declared `user_owned` in metaphor.codegen.yaml — see catalog_write_service.rs, which orchestrates
// them.
pub use attribute_repository::NewAttributeRow;
pub use attribute_value_repository::{AttributeValueResolveRow, NewAttributeValueRow};
pub use brand_repository::NewBrandRow;
pub use item_repository::{ItemHit, NewItemRow};
pub use item_group_repository::NewItemGroupRow;
pub use item_variant_repository::NewItemVariantRow;
pub use uom_repository::NewUomRow;
