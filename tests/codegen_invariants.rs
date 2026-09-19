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

/// Every feature this crate declares must be buildable.
///
/// The generator writes `#[cfg(feature = "openapi")] use utoipa::ToSchema;` into
/// each entity and DTO, but it does not add the dependency that import needs. A
/// crate that declares `openapi` without declaring `utoipa` compiles by default
/// and fails the moment anyone turns the feature on, which is how the OpenAPI
/// surface can sit broken while every routine check stays green. `cargo check
/// --all-features` is the leg that catches it; this test is the cheap guard that
/// runs on every `cargo test`.
#[test]
fn every_feature_that_needs_a_dependency_declares_it() {
    let manifest = include_str!("../Cargo.toml");

    let uses_utoipa = source_files_mentioning("utoipa::");
    if uses_utoipa.is_empty() {
        return;
    }

    assert!(
        manifest.lines().any(|l| l.trim_start().starts_with("utoipa")),
        "{} source file(s) import utoipa behind the `openapi` feature (first: {}), \
         so Cargo.toml must declare the dependency. Without it `cargo check \
         --all-features` fails on an unresolved import.",
        uses_utoipa.len(),
        uses_utoipa[0]
    );

    // The schema derive needs a companion feature for every foreign type it has
    // to describe. `rust_decimal` is the one this module's DTOs carry.
    if !source_files_mentioning("Decimal").is_empty() {
        assert!(
            manifest
                .lines()
                .filter(|l| l.trim_start().starts_with("utoipa"))
                .any(|l| l.contains("decimal")),
            "DTOs expose rust_decimal values, so the utoipa dependency must enable \
             its `decimal` feature or the derive cannot describe them."
        );
    }
}

/// Paths under `src/` whose text contains `needle`.
fn source_files_mentioning(needle: &str) -> Vec<String> {
    fn walk(dir: &std::path::Path, needle: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path)
                    .map(|s| s.contains(needle))
                    .unwrap_or(false)
            {
                out.push(path.display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new("src"), needle, &mut out);
    out.sort();
    out
}
