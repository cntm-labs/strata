# Strata Project Tooling & CI/CD Design

**Goal:** Set up production-grade Rust tooling, CI/CD, pre-commit hooks, code coverage, and code quality automation — adapted from nucleus and neuron-kernel patterns for Strata's scope.

**Approach:** Single crate with strict lint policy, 5 CI workflows, pre-commit hooks, codecov integration.

---

## 1. Rust Tooling Config Files

### rustfmt.toml (update existing)
```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
```

### .clippy.toml
```toml
msrv = "1.83.0"
disallowed-names = ["foo", "bar", "baz", "tmp", "temp"]
too-many-arguments-threshold = 10
```

### .cargo/config.toml
```toml
[alias]
check-all = "check --workspace --all-targets"
test-all = "test --workspace"
lint = "clippy --workspace --all-targets -- -D warnings"
```

### deny.toml
Dependency license + advisory scanning. Allow MIT/Apache-2.0/BSD/ISC/Unicode/Zlib/BSL/0BSD.

### rust-toolchain.toml
Pin stable channel with rustfmt, clippy, llvm-tools-preview components.

### bacon.toml
Watch-mode runner with keybindings: c=clippy, t=test, f=fmt.

## 2. Workspace Lint Policy

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
dead_code = "deny"
unused_imports = "deny"
unused_variables = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"

[profile.release]
strip = true
lto = true
codegen-units = 1
```

## 3. CI/CD Workflows

### ci.yml — Main Pipeline
- **fmt:** `cargo fmt --all -- --check`
- **clippy:** `cargo clippy --workspace -- -D warnings`
- **test:** `cargo test --workspace` (PostgreSQL service container)
- **coverage:** `cargo llvm-cov` → upload to Codecov
- **dashboard:** `npm ci && npm run build` (includes typecheck)

### security.yml — Dependency Scanning
- **cargo-audit:** Vulnerability scanning
- **cargo-deny:** License + advisory checks

### claude.yml — AI PR Review
- Triggered on PR open/sync
- Uses `CLAUDE_CODE_OAUTH_TOKEN` secret
- Structured review: security, tests, performance, code quality

### release.yml — Docker Build & Publish
- Triggered on `v*` tags
- Multi-stage Docker build
- Push to ghcr.io

### codecov.yml — Coverage Config
- Project target: auto, threshold 5%
- Patch target: 70%
- Ignore: benches, migrations, dashboard

## 4. Pre-commit Hook

`scripts/pre-commit`:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cd dashboard && npx prettier --check "src/**/*.{ts,vue}"`

`scripts/setup-hooks.sh`: Installs pre-commit hook.

## 5. Frontend Quality

- Prettier (already configured by Vue scaffold)
- ESLint (already configured)
- TypeScript strict via `vue-tsc --build`

## 6. Not Included (future)

- release-please, sdk-publish, load-test, weekly-digest, issue-triage
- k6 load tests, k8s manifests, Grafana dashboards
