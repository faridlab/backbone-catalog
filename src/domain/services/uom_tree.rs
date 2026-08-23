//! UoM parent-store tree: conversion math — hand-authored (user-owned).
//!
//! Pure, application-side conversion over the unit tree (ADR-0023):
//!
//! - Every unit carries a stored `factor`: the recursive product of its `relative_factor`
//!   chain down to its tree's root, re-derived on every tree write by
//!   `catalog.uom_recompute_factors()`. A root's factor is 1.
//! - Converting a quantity from unit A to unit B in the SAME tree is
//!   `qty * factor_A / factor_B` — the stored factors make reads cheap; no SQL runs
//!   during conversion (Odoo converts ORM-side, never SQL-side; the port keeps that).
//! - Units in DIFFERENT trees share no reference root. The conversion FAILS LOUDLY with a
//!   typed error naming both trees — never a silent numeric result. This subsumes Odoo's
//!   opt-in `_has_common_reference` pre-check: the guard lives in the primitive, so no
//!   caller can forget it.
//! - Rounding is part of the primitive and is declared by the caller (`ConversionRounding`)
//!   — round by VALUE with an explicit policy, never guessed from a unit's precision
//!   label (the rounding-by-value lesson carried over from the tax engine).
//!
//! The pre-v19 category model (`uom.category`, `factor_inv`) stays deleted per ADR-0023:
//! a category is just a two-level tree, and the inverse factor is derivation (1/factor),
//! not a stored column.

use rust_decimal::{Decimal, RoundingStrategy};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Caller-declared rounding policy for a converted quantity.
///
/// The converter never guesses: an explicit policy is an argument, and `Exact` is the
/// honest default for callers that round at their own boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionRounding {
    /// Return the exact converted value with no rounding applied.
    Exact,
    /// Round the converted value to `dp` decimal places using `strategy`
    /// (e.g. `MidpointAwayFromZero`, the money-grade half-up).
    Places {
        dp: u32,
        strategy: RoundingStrategy,
    },
}

impl ConversionRounding {
    /// Apply this policy to an exact converted quantity (round by value).
    pub fn apply(self, d: Decimal) -> Decimal {
        match self {
            ConversionRounding::Exact => d,
            ConversionRounding::Places { dp, strategy } => d.round_dp_with_strategy(dp, strategy),
        }
    }
}

/// One unit row of a loaded unit-to-root chain.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct UomChainNode {
    pub id: Uuid,
    pub code: String,
    /// Parent (reference) unit — `None` marks a tree root.
    pub relative_uom_id: Option<Uuid>,
    /// Ratio to the parent: 1 of this unit = `relative_factor` of the parent
    /// (`None` for a root).
    pub relative_factor: Option<Decimal>,
    /// Recursive stored effective factor to the tree's root (1 of this unit =
    /// `factor` of the root unit; 1 for a root itself).
    pub factor: Decimal,
}

/// A unit's ancestry: the leaf unit first, then each parent, the tree root last.
#[derive(Debug, Clone, PartialEq)]
pub struct UomChain {
    nodes: Vec<UomChainNode>,
}

impl UomChain {
    /// Assemble a chain from an unordered set of tree rows by walking parent links
    /// starting at `leaf_id`. Fails loudly when a parent link points outside the loaded
    /// set (dangling) or when the walk revisits a unit (a cycle among the stored links).
    pub fn from_rows(
        leaf_id: Uuid,
        rows: Vec<UomChainNode>,
    ) -> Result<Self, UomConversionError> {
        let by_id: HashMap<Uuid, UomChainNode> =
            rows.into_iter().map(|n| (n.id, n)).collect();
        let leaf = by_id.get(&leaf_id).ok_or_else(|| UomConversionError::DanglingRelative {
            code: leaf_id.to_string(),
            relative_uom_id: leaf_id,
        })?;
        let mut nodes = Vec::with_capacity(by_id.len());
        let mut seen: HashSet<Uuid> = HashSet::new();
        let mut cursor = leaf;
        loop {
            if !seen.insert(cursor.id) {
                return Err(UomConversionError::CycleDetected { code: cursor.code.clone() });
            }
            let next = match cursor.relative_uom_id {
                None => {
                    nodes.push(cursor.clone());
                    break;
                }
                Some(parent_id) => {
                    nodes.push(cursor.clone());
                    match by_id.get(&parent_id) {
                        Some(parent) => parent,
                        None => {
                            return Err(UomConversionError::DanglingRelative {
                                code: cursor.code.clone(),
                                relative_uom_id: parent_id,
                            })
                        }
                    }
                }
            };
            cursor = next;
        }
        Ok(Self { nodes })
    }

    /// The leaf unit this chain was loaded from.
    pub fn leaf(&self) -> &UomChainNode {
        &self.nodes[0]
    }

    /// The tree's root unit. Fails loudly if the stored links never reach a root
    /// (cycle — the loader's walk should have caught it first; this is the backstop).
    pub fn root(&self) -> Result<&UomChainNode, UomConversionError> {
        let last = self.nodes.last().expect("a chain is never empty");
        if last.relative_uom_id.is_some() {
            return Err(UomConversionError::CycleDetected { code: last.code.clone() });
        }
        Ok(last)
    }

    /// The leaf's factor to the tree root, re-derived by multiplying the
    /// `relative_factor` links hop by hop. The stored `factor` must always agree with
    /// this — the golden tests assert that agreement.
    pub fn derived_root_factor(&self) -> Result<Decimal, UomConversionError> {
        self.root()?;
        let mut f = Decimal::ONE;
        for n in &self.nodes {
            f *= n.relative_factor.unwrap_or(Decimal::ONE);
        }
        Ok(f)
    }
}

/// Typed conversion failure. Cross-tree conversion is NEVER a silent numeric result —
/// the error names both trees so the caller can surface exactly what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UomConversionError {
    /// The two units share no reference root — they live in different trees.
    CrossTree {
        from_code: String,
        from_root_code: String,
        to_code: String,
        to_root_code: String,
    },
    /// The stored parent links never reach a root (cycle).
    CycleDetected { code: String },
    /// A parent link points at a unit that is not loaded / not visible.
    DanglingRelative { code: String, relative_uom_id: Uuid },
}

impl UomConversionError {
    /// Stable machine-readable code (mirrors the CatalogWriteError `code()` convention).
    pub fn code(&self) -> &'static str {
        match self {
            UomConversionError::CrossTree { .. } => "cross_tree_conversion",
            UomConversionError::CycleDetected { .. } => "uom_tree_cycle",
            UomConversionError::DanglingRelative { .. } => "dangling_relative_uom",
        }
    }
}

impl std::fmt::Display for UomConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UomConversionError::CrossTree {
                from_code,
                from_root_code,
                to_code,
                to_root_code,
            } => write!(
                f,
                "cannot convert {from_code} -> {to_code}: the units live in different trees \
                 ({from_code} is in the tree rooted at {from_root_code}, {to_code} is in the \
                 tree rooted at {to_root_code}) — there is no common reference unit"
            ),
            UomConversionError::CycleDetected { code } => {
                write!(f, "unit tree cycle detected at or above {code}: the parent links never reach a root")
            }
            UomConversionError::DanglingRelative { code, relative_uom_id } => {
                write!(f, "unit {code} links at relative_uom_id {relative_uom_id}, which is not visible in this scope")
            }
        }
    }
}

impl std::error::Error for UomConversionError {}

/// Convert `qty` of the leaf unit of `from` into the leaf unit of `to`, over their
/// stored tree factors, applying the caller-declared rounding policy.
///
/// Same unit converts to itself unchanged (rounding still applies, so an `Exact`
/// identity is bitwise-identical). Cross-tree pairs return
/// [`UomConversionError::CrossTree`] — the loud replacement for Odoo's silent
/// multiply-by-nonsense and its opt-in `_has_common_reference` check.
pub fn convert_quantity(
    qty: Decimal,
    from: &UomChain,
    to: &UomChain,
    rounding: ConversionRounding,
) -> Result<Decimal, UomConversionError> {
    let from_root = from.root()?;
    let to_root = to.root()?;
    if from_root.id != to_root.id {
        return Err(UomConversionError::CrossTree {
            from_code: from.leaf().code.clone(),
            from_root_code: from_root.code.clone(),
            to_code: to.leaf().code.clone(),
            to_root_code: to_root.code.clone(),
        });
    }
    if from.leaf().id == to.leaf().id {
        return Ok(rounding.apply(qty));
    }
    // Same tree: qty_in_root = qty * factor_from ; qty_to = qty_in_root / factor_to.
    // Division by zero is unrepresentable: every stored factor is constrained > 0.
    Ok(rounding.apply(qty * from.leaf().factor / to.leaf().factor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: Uuid, code: &str, parent: Option<Uuid>, rf: Option<Decimal>, factor: Decimal) -> UomChainNode {
        UomChainNode { id, code: code.into(), relative_uom_id: parent, relative_factor: rf, factor }
    }

    // Pure-math checks that need no database: same-unit identity, chain assembly from
    // unordered rows, and cycle/dangling loudness. The SQL-backed golden cases
    // (recursion, cross-tree refusal, rounding, idempotent recompute) live in
    // tests/uom_tree_golden_cases.rs.

    #[test]
    fn same_unit_converts_identically_under_exact() {
        let id = Uuid::new_v4();
        let chain = UomChain::from_rows(id, vec![node(id, "PCS", None, None, Decimal::ONE)]).unwrap();
        let q = Decimal::new(1234, 2);
        assert_eq!(
            convert_quantity(q, &chain, &chain, ConversionRounding::Exact).unwrap(),
            q
        );
    }

    #[test]
    fn chain_assembles_from_unordered_rows_leaf_first_root_last() {
        let unit = Uuid::new_v4();
        let pack = Uuid::new_v4();
        let box_ = Uuid::new_v4();
        // deliberately out of order
        let rows = vec![
            node(pack, "PACK", Some(unit), Some(Decimal::from(10)), Decimal::from(10)),
            node(box_, "BOX", Some(pack), Some(Decimal::from(12)), Decimal::from(120)),
            node(unit, "UNIT", None, None, Decimal::ONE),
        ];
        let chain = UomChain::from_rows(box_, rows).unwrap();
        assert_eq!(chain.leaf().code, "BOX");
        assert_eq!(chain.root().unwrap().code, "UNIT");
        assert_eq!(chain.nodes.len(), 3);
        assert_eq!(chain.derived_root_factor().unwrap(), Decimal::from(120));
    }

    #[test]
    fn cycle_in_stored_links_fails_loudly() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let rows = vec![
            node(a, "A", Some(b), Some(Decimal::ONE), Decimal::ONE),
            node(b, "B", Some(a), Some(Decimal::ONE), Decimal::ONE),
        ];
        let err = UomChain::from_rows(a, rows).unwrap_err();
        assert!(matches!(err, UomConversionError::CycleDetected { .. }));
    }

    #[test]
    fn dangling_parent_link_fails_loudly() {
        let a = Uuid::new_v4();
        let ghost = Uuid::new_v4();
        let rows = vec![node(a, "A", Some(ghost), Some(Decimal::ONE), Decimal::ONE)];
        let err = UomChain::from_rows(a, rows).unwrap_err();
        assert!(matches!(err, UomConversionError::DanglingRelative { .. }));
    }
}
