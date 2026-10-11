// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Generated on disk but not declared by metaphor-schema (generator gap).
// Declared here so regeneration preserves the wiring.
pub mod usecases;          // per-entity use-case orchestration over services + DTOs
pub mod auth;              // per-entity RBAC guards
pub mod bulk_operations;   // per-entity bulk create/update helpers
// NOTE: `subscriptions` is intentionally NOT declared — its generated
// registry.rs has an incomplete type alias (`SubscriberRegistry` needs a
// concrete `<Event>` generic the generator never filled in). Declare it once a
// concrete catalog event type is wired; as-is it does not compile.
