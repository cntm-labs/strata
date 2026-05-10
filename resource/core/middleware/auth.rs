//! Nucleus JWT auth middleware.
//!
//! This is Strata's hand-rolled tower middleware around `cntm-nucleus`'s
//! `NucleusClient::verify_token`. We do NOT use the SDK's `axum` feature
//! because `cntm-nucleus 0.3.0` ships an `axum::FromRequestParts` impl
//! whose explicit lifetime parameters don't match the trait signature in
//! `axum-core 0.5.6` (filed: cntm-labs/nucleus#95). Once that lands we can
//! drop this file and use `nucleus_rs::axum::NucleusLayer` directly.
//!
//! When `NUCLEUS_API_KEY` is unset the layer is not installed (see
//! `build_router` in `main.rs`), and protected routes serve as the default
//! tenant — preserving the dev path.

use std::sync::Arc;

use axum::{
    extract::{Extension, Request},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use cntm_nucleus::NucleusClient;

/// Verify the `Authorization: Bearer <token>` header against Nucleus's JWKS
/// endpoint. On success, insert the parsed `NucleusClaims` into request
/// extensions so downstream middleware (notably `inject_tenant`) can read
/// them. On failure, short-circuit with `401 Unauthorized`.
pub async fn require_auth(
    Extension(client): Extension<Arc<NucleusClient>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = client.verify_token(token).await.map_err(|e| {
        tracing::debug!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
