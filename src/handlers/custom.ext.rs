//! The extension of `custom.rs` (hand-written; ADR-0031).

#[allow(unused_imports)]
use super::*;


/// Example custom handler demonstrating AppState usage.
///
/// # Example
///
/// ```ignore
/// // In routes/mod.rs:
/// Router::new().route("/health", get(health_check))
/// ```
#[allow(dead_code)]
pub async fn health_check(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // Access services via _state if needed
    // e.g., _state.user_service.health_check().await
    (StatusCode::OK, "OK")
}

// Add more custom handlers below...
