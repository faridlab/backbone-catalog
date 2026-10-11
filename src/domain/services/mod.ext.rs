// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// UoM parent-store tree conversion math (ADR-0023) — hand-authored, user-owned
// (declared in metaphor.codegen.yaml). Declared here inside the CUSTOM block so
// regeneration preserves the wiring.
pub mod uom_tree;
pub use uom_tree::{convert_quantity, ConversionRounding, UomChain, UomChainNode, UomConversionError};
