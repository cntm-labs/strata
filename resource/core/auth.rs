use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NucleusClaims {
    pub sub: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    pub exp: u64,
    pub iat: u64,
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let secret_key = state
        .config
        .nucleus_secret_key
        .as_deref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::ES256);
    validation.set_required_spec_claims(&["sub", "exp", "iat"]);

    let token_data = decode::<NucleusClaims>(
        token,
        &DecodingKey::from_secret(secret_key.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        tracing::debug!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    req.extensions_mut().insert(token_data.claims);

    Ok(next.run(req).await)
}
