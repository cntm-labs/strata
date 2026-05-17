//! Tenant middleware. Reads the verified Nucleus claims (inserted by
//! `middleware::auth::require_auth`) and injects a [`TenantId`] into the
//! request extensions for downstream RLS-aware handlers.
//!
//! When `org_id` is a previously-unseen tenant, just-in-time provisions a
//! `tenants` row before the request reaches its handler so subsequent RLS
//! writes pass `WITH CHECK`. Race-safe via `INSERT … ON CONFLICT (id) DO
//! NOTHING`. Provisioning failure degrades to the default tenant rather
//! than 500ing the request — Sentry captures the ERROR via tracing.
//!
//! When the auth layer isn't installed (dev mode, no `NUCLEUS_API_KEY`)
//! or the user has no `org_id` claim, falls back to the default tenant
//! (`Uuid::nil()`) seeded in migration 005.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use cntm_nucleus::claims::NucleusClaims;
use uuid::Uuid;

use crate::db::TenantId;
use crate::AppState;

pub async fn inject_tenant(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let claims = req.extensions().get::<NucleusClaims>().cloned();

    let tenant_id = resolve_tenant_id(&state, claims.as_ref()).await;

    req.extensions_mut().insert(TenantId(tenant_id));
    next.run(req).await
}

async fn resolve_tenant_id(state: &AppState, claims: Option<&NucleusClaims>) -> Uuid {
    let org_id_str = match claims.and_then(|c| c.org_id.as_deref()) {
        Some(s) => s,
        None => return Uuid::nil(),
    };

    let uuid = match Uuid::parse_str(org_id_str) {
        Ok(u) => u,
        Err(_) => return Uuid::nil(),
    };

    let (name, slug) = derive_tenant_identifiers(uuid, claims);

    match provision_tenant(&state.pool, uuid, &name, &slug).await {
        Ok(()) => uuid,
        Err(e) => {
            tracing::error!(
                org_id = %uuid,
                error = %e,
                "tenant provisioning failed; falling back to default tenant"
            );
            Uuid::nil()
        }
    }
}

/// Derive `(name, slug)` for a `tenants` row.
///
/// - `claims.org_slug = Some(s)` → both fields use `s`.
/// - `claims.org_slug = None` → slug is the full UUID string (collision-proof
///   because the PK is also that UUID), name is `"Org <first 8 of UUID>"`.
fn derive_tenant_identifiers(uuid: Uuid, claims: Option<&NucleusClaims>) -> (String, String) {
    match claims.and_then(|c| c.org_slug.clone()) {
        Some(slug) => (slug.clone(), slug),
        None => {
            let uuid_str = uuid.to_string();
            let short: String = uuid_str.chars().take(8).collect();
            (format!("Org {short}"), uuid_str)
        }
    }
}

async fn provision_tenant(
    pool: &sqlx::PgPool,
    id: Uuid,
    name: &str,
    slug: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .execute(pool)
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn test_state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: None,
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        }
    }

    /// Build a tiny router that captures the TenantId set by `inject_tenant`
    /// into a shared mutex so tests can assert on it.
    fn capture_app(state: AppState) -> (axum::Router, Arc<Mutex<Option<TenantId>>>) {
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
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                inject_tenant,
            ))
            .with_state(state);
        (app, captured)
    }

    fn make_claims(org_id: Option<String>, org_slug: Option<String>) -> NucleusClaims {
        NucleusClaims {
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
            org_slug,
            org_role: None,
            org_permissions: None,
        }
    }

    async fn count_tenants(pool: &sqlx::PgPool) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tenants")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn tenant_default_when_no_claims_extension(pool: sqlx::PgPool) {
        let before = count_tenants(&pool).await;
        let (app, captured) = capture_app(test_state(pool.clone()));
        let resp = app
            .oneshot(HttpRequest::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
        // No claims → no provisioning → no new tenant rows.
        assert_eq!(count_tenants(&pool).await, before);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn tenant_default_when_org_id_unparseable(pool: sqlx::PgPool) {
        let before = count_tenants(&pool).await;
        let claims = make_claims(Some("not-a-uuid".into()), None);

        let (app, captured) = capture_app(test_state(pool.clone()));
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
        assert_eq!(count_tenants(&pool).await, before);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn tenant_default_when_org_id_missing(pool: sqlx::PgPool) {
        let before = count_tenants(&pool).await;
        let claims = make_claims(None, None);

        let (app, captured) = capture_app(test_state(pool.clone()));
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;
        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
        assert_eq!(count_tenants(&pool).await, before);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn provisions_tenant_for_new_org_id(pool: sqlx::PgPool) {
        let target = Uuid::new_v4();
        let claims = make_claims(Some(target.to_string()), Some("acme".into()));

        let (app, captured) = capture_app(test_state(pool.clone()));
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;

        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, target);

        let row: (String, String) = sqlx::query_as("SELECT name, slug FROM tenants WHERE id = $1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0, "acme");
        assert_eq!(row.1, "acme");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn idempotent_on_repeat_login(pool: sqlx::PgPool) {
        let target = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(target)
            .bind("preexisting")
            .bind(format!("pre-{target}"))
            .execute(&pool)
            .await
            .unwrap();
        let before = count_tenants(&pool).await;

        let claims = make_claims(Some(target.to_string()), Some(format!("pre-{target}")));
        let (app, captured) = capture_app(test_state(pool.clone()));
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;

        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, target);
        // ON CONFLICT DO NOTHING — no extra row, and the original name is
        // preserved (we don't UPDATE on conflict).
        assert_eq!(count_tenants(&pool).await, before);
        let name: String = sqlx::query_scalar("SELECT name FROM tenants WHERE id = $1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "preexisting");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn falls_back_to_uuid_slug_when_org_slug_missing(pool: sqlx::PgPool) {
        let target = Uuid::new_v4();
        let claims = make_claims(Some(target.to_string()), None);

        let (app, captured) = capture_app(test_state(pool.clone()));
        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;

        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, target);

        let row: (String, String) = sqlx::query_as("SELECT name, slug FROM tenants WHERE id = $1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
        let target_str = target.to_string();
        let short: String = target_str.chars().take(8).collect();
        assert_eq!(row.0, format!("Org {short}"));
        assert_eq!(row.1, target_str);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn provisioning_db_error_falls_back_to_default(pool: sqlx::PgPool) {
        let target = Uuid::new_v4();
        let claims = make_claims(Some(target.to_string()), Some("acme".into()));

        let state = test_state(pool.clone());
        let (app, captured) = capture_app(state);
        // Close the pool before the request is processed. provision_tenant's
        // pool.execute will fail; the middleware must log and fall back to
        // Uuid::nil() instead of 500ing.
        pool.close().await;

        let mut req = HttpRequest::get("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(claims);
        let resp = app.oneshot(req).await.unwrap();
        let _ = resp.into_body().collect().await;

        let got = captured.lock().unwrap().expect("tenant should be set");
        assert_eq!(got.0, Uuid::nil());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_provisioning_does_not_duplicate(pool: sqlx::PgPool) {
        let target = Uuid::new_v4();
        let before = count_tenants(&pool).await;

        // Drive two concurrent provisioning attempts at the SAME org_id.
        // ON CONFLICT (id) DO NOTHING must serialize them on the primary
        // key index so exactly one row ends up in tenants.
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let pool = pool.clone();
            tasks.push(tokio::spawn(async move {
                provision_tenant(&pool, target, "acme", "acme").await
            }));
        }
        for t in tasks {
            t.await.unwrap().unwrap();
        }

        assert_eq!(count_tenants(&pool).await, before + 1);
    }

    #[test]
    fn derive_uses_org_slug_when_present() {
        let uuid = Uuid::new_v4();
        let claims = make_claims(Some(uuid.to_string()), Some("acme-corp".into()));
        let (name, slug) = derive_tenant_identifiers(uuid, Some(&claims));
        assert_eq!(name, "acme-corp");
        assert_eq!(slug, "acme-corp");
    }

    #[test]
    fn derive_uses_uuid_when_org_slug_missing() {
        let uuid = Uuid::new_v4();
        let claims = make_claims(Some(uuid.to_string()), None);
        let (name, slug) = derive_tenant_identifiers(uuid, Some(&claims));
        let uuid_str = uuid.to_string();
        let short: String = uuid_str.chars().take(8).collect();
        assert_eq!(name, format!("Org {short}"));
        assert_eq!(slug, uuid_str);
    }
}
