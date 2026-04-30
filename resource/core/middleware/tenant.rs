use axum::{extract::Request, middleware::Next, response::Response};
use uuid::Uuid;

use crate::db::TenantId;

const MOCK_TENANT_ID: Uuid = Uuid::from_u128(0);

pub async fn inject_mock_tenant(mut req: Request, next: Next) -> Response {
    req.extensions_mut().insert(TenantId(MOCK_TENANT_ID));
    next.run(req).await
}
