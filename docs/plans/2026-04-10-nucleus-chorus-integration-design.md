# Nucleus Auth + Chorus Email Integration Design

## Goal

Add authentication (via Nucleus) and email alert notifications (via Chorus) to Strata.

## Decisions

- **All API routes require auth** except `/api/v1/health`
- **Nucleus SDK** (`cntm-nucleus` crate with `axum` feature) — handles JWT verification, JWKS caching, ES256 validation
- **Chorus embedded as library** (`chorus-rs` crate) — no separate Chorus server needed
- **Email only** via Resend provider — no SMS (not free)
- **No RBAC** — all authenticated users have full access
- **Single-tenant** — no org scoping

## Architecture

```
Browser → Vue 3 Frontend
           ├── /login → redirect to Nucleus OAuth
           ├── /auth/callback → store JWT in memory
           ↓ Authorization: Bearer <token>
         Axum Backend
           ├── /api/v1/health (no auth)
           └── NucleusLayer middleware
                 ↓ NucleusClaims
               API handlers
                 ↓ alert fires
               Chorus (embedded)
                 └── Resend (email)
```

## Backend

### Dependencies

```toml
cntm-nucleus = { version = "0.3", features = ["axum"] }
chorus-rs = { git = "https://github.com/cntm-labs/chorus.git", tag = "chorus-rs-v0.2.0" }
```

### Config (new env vars)

```
NUCLEUS_SECRET_KEY=sk_live_xxx       # Required for auth
NUCLEUS_BASE_URL=https://nucleus.example.com  # Optional, defaults to api.nucleus.dev
RESEND_API_KEY=re_xxx                # Required for email alerts
ALERT_FROM_EMAIL=alerts@strata.dev   # Default sender
```

### AppState changes

Add `NucleusClient` and `Chorus` to `AppState` so handlers can access them.

### Router structure

```rust
// No auth
Router::new()
    .route("/api/v1/health", get(health))

// Auth required — wrapped in NucleusLayer
Router::new()
    .nest("/api/v1/datasources", datasource_routes())
    .nest("/api/v1/dashboards", dashboard_routes())
    // ... all other routes
    .with_state(state)
    .layer(NucleusLayer::new(nucleus_client))
```

### Alert email flow

1. Alert fires (test-fire endpoint or future evaluator)
2. Check `notification_channels` contains `"email"`
3. For each recipient in `notification_recipients`:
   - Call `chorus.send_email(EmailMessage { to, subject, html_body, text_body, from })`
4. Record result in `alert_events.notified_via`

### JWT details

- Algorithm: ES256 (ECDSA P-256)
- JWKS endpoint: `GET <nucleus_url>/.well-known/jwks.json`
- Token lifetime: 5 minutes (short-lived)
- Claims available: `sub`, `email`, `first_name`, `last_name`, `org_id`, `org_role`

## Frontend

### Auth flow

1. User visits Strata → check if JWT exists in memory
2. If no JWT → redirect to `/login` → Nucleus OAuth flow
3. Nucleus redirects back to `/auth/callback` with auth code
4. Frontend exchanges code for JWT (via Nucleus token endpoint)
5. Store JWT in memory (not localStorage — XSS safety)
6. Attach `Authorization: Bearer <token>` to all `/api/v1/*` requests
7. On 401 response → redirect to `/login`

### New frontend routes

| Route | Component | Description |
|-------|-----------|-------------|
| `/login` | LoginView | Redirect to Nucleus OAuth |
| `/auth/callback` | AuthCallbackView | Handle OAuth callback |

### API client changes

- Add JWT token to fetch headers
- Add 401 interceptor that redirects to login
- Add auth state composable (`useAuth`)

### Sidebar changes

- Show user avatar + name from JWT claims
- Add logout button

## What we skip (YAGNI)

- SMS notifications
- Webhook delivery callbacks from Chorus
- Role-based access control
- Multi-tenant / org scoping
- Token refresh (short session, re-login on expiry)

## Testing

- Backend: mock `NucleusLayer` in existing tests (tests already work without auth)
- New tests: verify 401 without token, verify claims extraction
- Chorus: wiremock Resend API in alert notification tests
- Frontend: mock auth state in existing component tests
