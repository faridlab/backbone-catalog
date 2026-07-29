//! Codegen invariants — regression guards for hand-written safety gates that
//! `metaphor schema generate` does NOT emit and would otherwise strip on regen.
//!
//! This file is `user_owned` (metaphor.codegen.yaml), so it survives regen too.
//! If a test here fails, regen (or a manual edit) removed a safety gate — re-apply
//! it on the named source file and confirm that file is listed as `user_owned`.

/// The unvalidated generic-CRUD mounts must stay gated behind
/// `#[cfg(any(test, feature = "unguarded"))]` so they are unreachable unless a
/// consumer explicitly opts in (ADR-001/002; council rec #1).
///
/// `metaphor schema generate` does not emit these attributes — without this guard,
/// regen would silently re-expose the unvalidated CRUD surface by default.
#[test]
fn unguarded_crud_mounts_remain_feature_gated() {
    let lib = include_str!("../src/lib.rs");
    let routes = include_str!("../src/routes/mod.rs");
    let marker = "feature = \"unguarded\"";

    assert_eq!(
        lib.matches(marker).count(),
        2,
        "src/lib.rs must keep exactly 2 `unguarded` cfg gates (on all_crud_routes and routes). \
         If regen removed them, re-apply #[cfg(any(test, feature = \"unguarded\"))] above each \
         and ensure src/lib.rs is listed under user_owned in metaphor.codegen.yaml."
    );

    assert_eq!(
        routes.matches(marker).count(),
        4,
        "src/routes/mod.rs must keep exactly 4 `unguarded` cfg gates (on create_stateless_routes, \
         get_routes, create_combined_routes, get_routes_with_state). If regen removed them, \
         re-apply the attribute above each and ensure src/routes/mod.rs is user_owned."
    );
}
