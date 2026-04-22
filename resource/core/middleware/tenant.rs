use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::AppState;

pub async fn set_tenant_context(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    // For now, we use a mock tenant ID. 
    // In a real application, this might be extracted from NucleusClaims or a header.
    let tenant_id = "00000000-0000-0000-0000-000000000000";

    // Set the tenant_id in the PostgreSQL session.
    // This allows Row Level Security (RLS) to use this variable.
    let _ = sqlx::query(&format!("SET app.tenant_id = '{}'", tenant_id))
        .execute(&state.db)
        .await;

    next.run(req).await
}
