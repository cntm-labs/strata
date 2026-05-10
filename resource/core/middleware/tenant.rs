//! Tenant middleware. Reads the verified Nucleus claims (inserted by
//! `middleware::auth::require_auth`) and injects a [`TenantId`] into the
//! request extensions for downstream RLS-aware handlers.
//!
//! When the auth layer isn't installed (dev mode, no `NUCLEUS_API_KEY`)
//! or the user has no `org_id` claim, falls back to the default tenant
//! (`Uuid::nil()`) seeded in migration 005.

use axum::{extract::Request, middleware::Next, response::Response};
use uuid::Uuid;

use crate::db::TenantId;

pub async fn inject_tenant(mut req: Request, next: Next) -> Response {
    let tenant_id = req
        .extensions()
        .get::<cntm_nucleus::claims::NucleusClaims>()
        .and_then(|c| c.org_id.as_deref())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(Uuid::nil());

    req.extensions_mut().insert(TenantId(tenant_id));
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    /// Build a tiny router that captures the TenantId set by `inject_tenant`
    /// into a shared mutex so tests can assert on it.
    fn capture_app() -> (axum::Router, Arc<Mutex<Option<TenantId>>>) {
        let captured: Arc<Mutex<Option<TenantId>>> = Arc::new(Mutex::new(None));
        let captured_for_handler = Arc::clone(&captured);

        let handler = move |req: Request| {
            let captured = Arc::clone(&captured_for_handler);
            async move {
                if let Some(tid) = req.extensions().get::<TenantId>().copied() {
                    *captured.lock().unwrap() = Some(tid);
                }
                axum::http::Response::builder()
                    .status(200)
                    .body(axum::body::Body::empty())
                    .unwrap()
            }
        };

        let app = axum::Router::new()
            .route("/", axum::routing::get(handler))
            .layer(axum::middleware::from_fn(inject_tenant));
        (app, captured)
    }

    fn make_claims(org_id: Option<String>) -> cntm_nucleus::claims::NucleusClaims {
        cntm_nucleus::claims::NucleusClaims {
            sub: "user-1".into(),
            iss: "test".into(),
            aud: "strata".into(),
            exp: 9_999_999_999,
            iat: 0,
            jti: None,
            email: None,
            first_name: None,
            last_name: None,
            avatar_url: None,
            email_verified: None,
            metadata: None,
            org_id,
            org_slug: None,
            org_role: None,
            org_permissions: None,
        }
    }

    #[tokio::test]
    async fn tenant_default_when_no_claims_extension() {
        let (app, captured) = capture_app();
        let resp = app
            .oneshot(HttpRequest::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
    }

    #[tokio::test]
    async fn tenant_set_from_org_id_in_claims() {
        let target = Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);
        let claims = make_claims(Some(target.to_string()));

        let (app, captured) = capture_app();
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, target);
    }

    #[tokio::test]
    async fn tenant_default_when_org_id_unparseable() {
        let claims = make_claims(Some("not-a-uuid".to_string()));

        let (app, captured) = capture_app();
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
    }
}
