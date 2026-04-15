# CLAUDE.md — Strata Project Guide

## Overview
Strata is an open-source observability dashboard — a general-purpose Grafana alternative with full UI/UX control. Built with a Rust backend and Vue 3 frontend.

## Tech Stack
- **Backend:** Rust (stable) with Axum web framework
- **Frontend:** Vue 3 + TypeScript + Vite
- **UI Components:** PrimeVue
- **Styling:** TailwindCSS + DaisyUI
- **Charts:** Apache ECharts (vue-echarts) for general charts + uPlot for time series
- **Data Grid:** AG Grid Community (structured logs, tables)
- **Log Viewer:** xterm.js (raw log streaming)
- **Dashboard Layout:** vue-grid-layout (drag-and-drop panels)
- **Database:** PostgreSQL (dashboards, panels, alerts, user preferences)
- **Auth:** Nucleus (integrated, ES256 JWT via jsonwebtoken crate, optional via NUCLEUS_SECRET_KEY)
- **Alerting:** Chorus (email via Resend, embedded chorus-rs library, optional via RESEND_API_KEY)
- **Data Sources:** Prometheus (PromQL), Loki (LogQL), PostgreSQL (SQL)

## Build Commands

### Backend (Rust)
```sh
cd resource
cargo check                      # Type check
cargo test                       # Run tests
cargo clippy -- -D warnings      # Lint
cargo fmt --all                  # Format (uses rustfmt.toml)
```

### Frontend (Vue)
```sh
cd dashboard
npm install                      # Install dependencies
npm run dev                      # Dev server (Vite)
npm run build                    # Production build
npm run type-check               # TypeScript check
npm run lint                     # ESLint
npm run format                   # Prettier
```

## Project Structure
```
strata/
├── resource/                    # Rust backend (Axum)
│   ├── core/
│   │   ├── main.rs
│   │   ├── api/                 # REST API handlers
│   │   ├── datasource/          # Prometheus, Loki, PostgreSQL proxies
│   │   ├── auth/                # Nucleus integration (TODO)
│   │   ├── config/              # App config
│   │   ├── error/               # Error types
│   │   └── middleware/          # Auth, CORS
│   ├── Cargo.toml
│   └── rustfmt.toml
├── dashboard/                   # Vue 3 + TypeScript frontend
│   ├── src/
│   │   ├── views/               # Page components
│   │   ├── components/          # Reusable components
│   │   ├── composables/         # Vue composables
│   │   ├── stores/              # Pinia stores
│   │   ├── types/               # TypeScript types
│   │   └── api/                 # HTTP client
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   ├── eslint.config.js
│   └── .prettierrc
├── Cargo.toml                   # Workspace root
├── SITEMAP.md                   # All routes and API endpoints
├── Dockerfile
└── docker-compose.yml
```

## Quality Commands

### Rust
```sh
cargo lint                       # Clippy with -D warnings (alias)
cargo deny check                 # License + advisory check
cargo llvm-cov --workspace       # Code coverage
```

### Frontend
```sh
cd dashboard
npm run lint                     # ESLint
npm run format                   # Prettier
```

### Pre-commit
```sh
./scripts/setup-hooks.sh         # Install git hooks (one-time)
```

## Lint Policy
- `unsafe_code` = forbid (no unsafe Rust)
- `dead_code` = deny (remove unused code)
- `unused_imports` = deny (clean imports)
- `clippy::all` = warn (standard clippy lints)

## Key Design Decisions
- **No Grafana dependency** — full control over UI/UX and theming
- **Rust backend proxies all datasource queries** — never expose Prometheus/Loki directly to browser
- **Panel types:** timeseries (uPlot), stat, gauge, bar, heatmap, piechart (ECharts), table (AG Grid), logs (xterm.js)
- **Dashboard templates** — built-in templates for Chorus, Nucleus, infrastructure monitoring
- **Auth via Nucleus** — Strata does not manage users, delegates to Nucleus OAuth
- **Alerting via Chorus** — alert notifications sent through Chorus SMS/Email

## Conventions
- Backend errors use a unified error type with code/status/message
- All datasource credentials encrypted at rest
- Dashboard layout uses 12-column grid system
- Template variables use `{{variable}}` syntax
- Explore mode auto-detects query result type → renders appropriate panel

## Related Projects
- **Chorus** (github.com/cntm-labs/chorus) — CPaaS for alert notifications
- **Nucleus** (github.com/cntm-labs/nucleus) — Auth provider
- **Orbit** (github.com/cntm-labs/orbit-api) — Finance API (observed by Strata)
