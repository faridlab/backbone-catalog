// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod catalog_write_service;
pub use catalog_write_service::{
    CatalogWriteError, CatalogWriteService, NewAttribute, NewAttributeValue, NewBrand, NewItem,
    NewItemGroup, NewItemVariant, NewUom,
};
