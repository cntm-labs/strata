# Nucleus Auth + Chorus Email Integration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add JWT authentication (Nucleus) and email alert notifications (Chorus/Resend) to Strata.

**Architecture:** `NucleusLayer` Axum middleware protects all API routes except health. Chorus library embedded in AppState sends emails when alerts fire. Frontend stores JWT in memory, attaches Bearer token to all requests, redirects to Nucleus OAuth on 401.

**Tech Stack:** `cntm-nucleus` v0.3 (axum feature), `chorus-rs` v0.2.0 (git dep), Resend email provider, Vue 3 composables for auth state.

---

## PART 1: BACKEND

### Task 1: Add Nucleus + Chorus dependencies

**Files:**
- Modify: `resource/Cargo.toml`

**Step 1: Add dependencies**

Add to `[dependencies]` in `resource/Cargo.toml`:
```toml
cntm-nucleus = { version = "0.3", features = ["axum"] }
chorus-rs = { git = "https://github.com/cntm-labs/chorus.git", tag = "chorus-rs-v0.2.0" }
```

**Step 2: Verify it compiles**

Run: `cd resource && cargo check`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add resource/Cargo.toml Cargo.lock
git commit -m "chore: add cntm-nucleus and chorus-rs dependencies"
```

---

### Task 2: Extend AppConfig with Nucleus + Chorus settings

**Files:**
- Modify: `resource/core/config/mod.rs`

**Step 1: Add new config fields**

```rust
use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub nucleus_secret_key: Option<String>,
    pub nucleus_base_url: Option<String>,
    pub resend_api_key: Option<String>,
    pub alert_from_email: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://strata:secret@localhost:5432/strata".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            nucleus_secret_key: env::var("NUCLEUS_SECRET_KEY").ok(),
            nucleus_base_url: env::var("NUCLEUS_BASE_URL").ok(),
            resend_api_key: env::var("RESEND_API_KEY").ok(),
            alert_from_email: env::var("ALERT_FROM_EMAIL")
                .unwrap_or_else(|_| "alerts@strata.dev".to_string()),
        }
    }
}
```

Note: `nucleus_secret_key` and `resend_api_key` are `Option<String>` — auth and email are disabled when not configured (graceful degradation for dev mode).

**Step 2: Update config tests**

Update the `clone_works` test to include new fields. Update `from_env_reads_and_defaults` to verify defaults.

**Step 3: Verify**

Run: `cd resource && cargo test config::tests -- --nocapture`
Expected: all tests pass

**Step 4: Update .env.example**

Add to `.env.example`:
```
NUCLEUS_SECRET_KEY=sk_live_xxx
NUCLEUS_BASE_URL=https://nucleus.example.com
RESEND_API_KEY=re_xxx
ALERT_FROM_EMAIL=alerts@strata.dev
```

**Step 5: Commit**

```bash
git add resource/core/config/mod.rs .env.example
git commit -m "feat: add Nucleus and Chorus config fields"
```

---

### Task 3: Add Nucleus auth middleware to router

**Files:**
- Modify: `resource/core/main.rs`

**Step 1: Refactor build_router to apply NucleusLayer**

```rust
pub fn build_router(state: AppState) -> Router {
    // Health endpoint — no auth
    let public = Router::new().route("/api/v1/health", get(health));

    // All other API routes — auth required when configured
    let protected = Router::new()
        .nest("/api/v1/datasources", api::datasources::datasource_routes())
        .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
        .nest("/api/v1", api::panels::panel_routes_nested())
        .nest("/api/v1/explore", api::explore::explore_routes())
        .nest("/api/v1/alerts", api::alerts::alert_routes())
        .nest("/api/v1/templates", api::templates::template_routes())
        .with_state(state.clone());

    // Apply NucleusLayer only if secret key is configured
    let protected = if let Some(ref secret_key) = state.config.nucleus_secret_key {
        use cntm_nucleus::{NucleusClient, NucleusConfig, axum::NucleusLayer};
        let client = NucleusClient::new(NucleusConfig {
            secret_key: secret_key.clone(),
            base_url: state.config.nucleus_base_url.clone(),
            jwks_cache_ttl_secs: None,
        });
        protected.layer(NucleusLayer::new(client))
    } else {
        tracing::warn!("NUCLEUS_SECRET_KEY not set — running without authentication");
        protected
    };

    public
        .merge(protected)
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
```

Note: Auth is optional — when `NUCLEUS_SECRET_KEY` is not set, Strata runs without auth (dev mode). This keeps existing tests working without mock auth.

**Step 2: Verify compile**

Run: `cd resource && cargo check`
Expected: compiles

**Step 3: Verify existing tests still pass**

Run: `cd resource && cargo test`
Expected: all tests pass (they run without NUCLEUS_SECRET_KEY so no auth)

**Step 4: Commit**

```bash
git add resource/core/main.rs
git commit -m "feat: add Nucleus auth middleware (optional, enabled via NUCLEUS_SECRET_KEY)"
```

---

### Task 4: Add auth test — 401 without token

**Files:**
- Modify: `resource/core/main.rs` (tests section)

**Step 1: Add test**

Add in `#[cfg(test)] mod tests`:
```rust
#[sqlx::test]
async fn protected_route_requires_auth_when_configured(pool: sqlx::PgPool) {
    let state = AppState {
        db: pool,
        config: AppConfig {
            database_url: String::new(),
            host: "127.0.0.1".into(),
            port: 3000,
            nucleus_secret_key: Some("sk_test_fake".into()),
            nucleus_base_url: None,
            resend_api_key: None,
            alert_from_email: "test@test.com".into(),
        },
    };
    let app = build_router(state);
    let resp = app
        .oneshot(Request::get("/api/v1/dashboards").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

Also update `test_state` helper to include new config fields (all None/default).

**Step 2: Run tests**

Run: `cd resource && cargo test tests -- --nocapture`
Expected: all pass including the new 401 test

**Step 3: Commit**

```bash
git add resource/core/main.rs
git commit -m "test: verify 401 on protected routes when auth is configured"
```

---

### Task 5: Build Chorus email notifier module

**Files:**
- Create: `resource/core/notifier.rs`
- Modify: `resource/core/main.rs` (add `pub mod notifier`)

**Step 1: Create notifier module**

```rust
use chorus::prelude::*;
use chorus::providers::email::resend::ResendEmailSender;
use std::sync::Arc;

pub struct Notifier {
    chorus: Option<Chorus>,
}

impl Notifier {
    pub fn new(resend_api_key: Option<&str>, from_email: &str) -> Self {
        let chorus = resend_api_key.map(|key| {
            let resend = ResendEmailSender::new(key.to_string(), from_email.to_string());
            Chorus::builder()
                .add_email_provider(Arc::new(resend))
                .default_from_email(from_email.to_string())
                .build()
        });
        Self { chorus }
    }

    pub async fn send_alert_email(
        &self,
        to: &str,
        rule_name: &str,
        message: &str,
    ) -> Result<(), String> {
        let Some(ref chorus) = self.chorus else {
            tracing::warn!("Email not configured — skipping notification to {}", to);
            return Ok(());
        };

        chorus
            .send_email(&EmailMessage {
                to: to.to_string(),
                subject: format!("[Strata Alert] {}", rule_name),
                html_body: format!(
                    "<h2>Alert: {}</h2><p>{}</p><p><small>Sent by Strata</small></p>",
                    rule_name, message
                ),
                text_body: format!("Alert: {}\n\n{}", rule_name, message),
                from: None,
            })
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

impl Clone for Notifier {
    fn clone(&self) -> Self {
        // Chorus doesn't implement Clone — rebuild from config is not ideal.
        // For now, wrap in Arc at the AppState level instead.
        unimplemented!("Use Arc<Notifier> in AppState")
    }
}
```

Note: Since `Chorus` doesn't impl `Clone`, `Notifier` should be wrapped in `Arc` in `AppState`.

**Step 2: Add to AppState**

Update `AppState` in `main.rs`:
```rust
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub notifier: Arc<notifier::Notifier>,
}
```

Update `main()` to create notifier:
```rust
let notifier = Arc::new(notifier::Notifier::new(
    config.resend_api_key.as_deref(),
    &config.alert_from_email,
));

let state = AppState {
    db,
    config: config.clone(),
    notifier,
};
```

**Step 3: Update all `test_state` helpers across test files**

Every `test_state` / `test_app` function needs the new `notifier` field:
```rust
notifier: Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
```

Files to update: `main.rs`, `dashboards.rs`, `datasources.rs`, `panels.rs`, `alerts.rs`, `templates.rs`, `explore.rs`, `query.rs`

**Step 4: Verify**

Run: `cd resource && cargo test`
Expected: all tests pass

**Step 5: Commit**

```bash
git add resource/core/notifier.rs resource/core/main.rs resource/core/api/
git commit -m "feat: add Chorus email notifier module"
```

---

### Task 6: Wire email notifications into alert test-fire

**Files:**
- Modify: `resource/core/api/alerts.rs`

**Step 1: Send emails after alert fires**

In `test_fire_rule`, after creating the event, add email sending:

```rust
// After: let event = sqlx::query_as ... .fetch_one(&state.db).await?;

// Send email notifications if firing
if firing {
    let email_recipients: Vec<String> = rule
        .notification_recipients
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .filter(|r| r.contains('@'))
                .collect()
        })
        .unwrap_or_default();

    for recipient in &email_recipients {
        if let Err(e) = state
            .notifier
            .send_alert_email(
                recipient,
                &rule.name,
                &format!(
                    "{} = {:.2}, threshold {:.2} ({})",
                    rule.query, val, rule.threshold, rule.condition
                ),
            )
            .await
        {
            tracing::error!("Failed to send alert email to {}: {}", recipient, e);
        }
    }
}
```

**Step 2: Add test with wiremock Resend**

Add a test that verifies email sending is attempted when alert fires. Use wiremock to mock the Resend API endpoint.

**Step 3: Verify**

Run: `cd resource && cargo test api::alerts::tests -- --nocapture`
Expected: all pass

**Step 4: Commit**

```bash
git add resource/core/api/alerts.rs
git commit -m "feat: send email notifications when alert fires"
```

---

## PART 2: FRONTEND

### Task 7: Create auth composable

**Files:**
- Create: `dashboard/src/composables/useAuth.ts`

**Step 1: Write auth composable**

```typescript
import { ref, computed } from 'vue'

interface User {
  id: string
  email: string
  firstName?: string
  lastName?: string
  avatarUrl?: string
}

const token = ref<string | null>(null)
const user = ref<User | null>(null)

export function useAuth() {
  const isAuthenticated = computed(() => !!token.value)

  function setToken(jwt: string) {
    token.value = jwt
    // Decode payload (no verification — backend verifies)
    try {
      const payload = JSON.parse(atob(jwt.split('.')[1]))
      user.value = {
        id: payload.sub,
        email: payload.email,
        firstName: payload.first_name,
        lastName: payload.last_name,
        avatarUrl: payload.avatar_url,
      }
    } catch {
      user.value = null
    }
  }

  function clearToken() {
    token.value = null
    user.value = null
  }

  function getToken(): string | null {
    return token.value
  }

  return { token, user, isAuthenticated, setToken, clearToken, getToken }
}
```

**Step 2: Commit**

```bash
git add dashboard/src/composables/useAuth.ts
git commit -m "feat: add useAuth composable for JWT token management"
```

---

### Task 8: Add auth header to API client + 401 redirect

**Files:**
- Modify: `dashboard/src/api/client.ts`

**Step 1: Update request function**

```typescript
import { useAuth } from '@/composables/useAuth'

const BASE_URL = '/api/v1'

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const { getToken, clearToken } = useAuth()
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  const jwt = getToken()
  if (jwt) {
    headers['Authorization'] = `Bearer ${jwt}`
  }

  const res = await fetch(`${BASE_URL}${path}`, {
    headers,
    ...options,
  })

  if (res.status === 401) {
    clearToken()
    window.location.href = '/login'
    throw new Error('Unauthorized')
  }

  if (!res.ok) {
    const error = await res.json().catch(() => ({ message: res.statusText }))
    throw new Error(error.message || `HTTP ${res.status}`)
  }
  return res.json()
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body ? JSON.stringify(body) : undefined,
    }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'PUT',
      body: body ? JSON.stringify(body) : undefined,
    }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
```

**Step 2: Update client tests**

Update `dashboard/src/api/__tests__/client.test.ts` to mock `useAuth` and verify Authorization header is attached.

**Step 3: Commit**

```bash
git add dashboard/src/api/client.ts dashboard/src/api/__tests__/client.test.ts
git commit -m "feat: attach JWT to API requests, redirect to login on 401"
```

---

### Task 9: Add login + callback views and routes

**Files:**
- Create: `dashboard/src/views/LoginView.vue`
- Create: `dashboard/src/views/AuthCallbackView.vue`
- Modify: `dashboard/src/router/index.ts`

**Step 1: Create LoginView**

Redirects to Nucleus OAuth URL. Read `VITE_NUCLEUS_URL` and `VITE_NUCLEUS_PROJECT_ID` from env.

```vue
<template>
  <div class="flex items-center justify-center h-screen">
    <div class="text-center">
      <h1 class="text-2xl font-bold mb-4">Redirecting to login...</h1>
      <ProgressSpinner />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import ProgressSpinner from 'primevue/progressspinner'

onMounted(() => {
  const nucleusUrl = import.meta.env.VITE_NUCLEUS_URL || 'https://nucleus.example.com'
  const projectId = import.meta.env.VITE_NUCLEUS_PROJECT_ID || ''
  const redirectUri = `${window.location.origin}/auth/callback`
  window.location.href = `${nucleusUrl}/auth/sign-in?project_id=${projectId}&redirect_uri=${encodeURIComponent(redirectUri)}`
})
</script>
```

**Step 2: Create AuthCallbackView**

Extracts token from URL params, stores in auth state, redirects to dashboards.

```vue
<template>
  <div class="flex items-center justify-center h-screen">
    <ProgressSpinner />
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuth } from '@/composables/useAuth'
import ProgressSpinner from 'primevue/progressspinner'

const router = useRouter()
const { setToken } = useAuth()

onMounted(() => {
  const params = new URLSearchParams(window.location.search)
  const token = params.get('token') || params.get('access_token')
  if (token) {
    setToken(token)
    router.replace('/dashboards')
  } else {
    router.replace('/login')
  }
})
</script>
```

**Step 3: Add routes**

Add to `router/index.ts` BEFORE the `'/'` route:
```typescript
{
  path: '/login',
  name: 'login',
  component: () => import('@/views/LoginView.vue'),
},
{
  path: '/auth/callback',
  name: 'auth-callback',
  component: () => import('@/views/AuthCallbackView.vue'),
},
```

**Step 4: Add route guard**

Add navigation guard to redirect unauthenticated users:
```typescript
import { useAuth } from '@/composables/useAuth'

router.beforeEach((to) => {
  const { isAuthenticated } = useAuth()
  const publicRoutes = ['login', 'auth-callback']
  if (!publicRoutes.includes(to.name as string) && !isAuthenticated.value) {
    return { name: 'login' }
  }
})
```

Note: Guard only activates when Nucleus is configured. In dev mode without NUCLEUS_SECRET_KEY, backend doesn't require auth, so frontend should also skip. Use `VITE_AUTH_ENABLED=true` env var to toggle.

**Step 5: Commit**

```bash
git add dashboard/src/views/LoginView.vue dashboard/src/views/AuthCallbackView.vue dashboard/src/router/index.ts
git commit -m "feat: add login, callback views and auth route guard"
```

---

### Task 10: Add user info to sidebar + logout

**Files:**
- Modify: `dashboard/src/layouts/AppSidebar.vue`

**Step 1: Add user section to sidebar**

Add at the bottom of the sidebar template, before closing `</aside>`:
```vue
<div v-if="user" class="p-4 border-t border-base-300">
  <div class="flex items-center gap-3">
    <div class="avatar placeholder">
      <div class="bg-neutral text-neutral-content w-8 rounded-full">
        <img v-if="user.avatarUrl" :src="user.avatarUrl" />
        <span v-else>{{ initials }}</span>
      </div>
    </div>
    <div class="flex-1 min-w-0">
      <div class="text-sm font-medium truncate">{{ user.firstName || user.email }}</div>
    </div>
    <button class="btn btn-ghost btn-xs" @click="logout">
      <i class="pi pi-sign-out" />
    </button>
  </div>
</div>
```

In script: import `useAuth`, compute `initials`, add `logout` function.

**Step 2: Commit**

```bash
git add dashboard/src/layouts/AppSidebar.vue
git commit -m "feat: show user info and logout button in sidebar"
```

---

### Task 11: Update frontend tests

**Files:**
- Create: `dashboard/src/composables/__tests__/useAuth.test.ts`
- Modify: `dashboard/src/api/__tests__/client.test.ts`
- Modify: `dashboard/src/router/__tests__/index.test.ts`
- Modify: `dashboard/src/layouts/__tests__/AppSidebar.test.ts`

**Step 1: Write useAuth tests**

Test: setToken decodes JWT, clearToken clears state, getToken returns token, isAuthenticated computed.

**Step 2: Update client tests**

Mock `useAuth` to return a token, verify `Authorization: Bearer` header is sent.

**Step 3: Update router tests**

Add login + auth-callback to expected routes list.

**Step 4: Update sidebar tests**

Verify logout button renders when user is present.

**Step 5: Run all frontend tests**

Run: `cd dashboard && bunx vitest run`
Expected: all pass

**Step 6: Commit**

```bash
git add dashboard/src/
git commit -m "test: add tests for auth composable, update client and router tests"
```

---

### Task 12: Update CLAUDE.md, SITEMAP.md, .env.example

**Files:**
- Modify: `CLAUDE.md`
- Modify: `SITEMAP.md`
- Modify: `.env.example`

**Step 1: Update CLAUDE.md**

- Change `Auth: Nucleus (TODO)` to `Auth: Nucleus (integrated, ES256 JWT via cntm-nucleus crate)`
- Change `Alerting: Chorus (TODO)` to `Alerting: Chorus (email via Resend, embedded chorus-rs library)`

**Step 2: Update SITEMAP.md**

Add `/login` and `/auth/callback` frontend routes. Update totals.

**Step 3: Verify lint/format**

Run: `cargo clippy -- -D warnings && cargo fmt --all --check`
Run: `cd dashboard && bunx eslint . && bunx prettier --check src/`

**Step 4: Commit**

```bash
git add CLAUDE.md SITEMAP.md .env.example
git commit -m "docs: update CLAUDE.md and SITEMAP.md for Nucleus + Chorus integration"
```

---

### Task 13: Final verification

**Step 1: Run full backend tests**

Run: `cd resource && cargo test`
Expected: all pass

**Step 2: Run full frontend tests**

Run: `cd dashboard && bunx vitest run`
Expected: all pass

**Step 3: Run lint/format**

Run: `cargo clippy -- -D warnings && cargo fmt --all --check`
Run: `cd dashboard && bunx eslint . && bunx prettier --check src/`
Expected: clean

**Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "feat: complete Nucleus auth + Chorus email integration"
```
