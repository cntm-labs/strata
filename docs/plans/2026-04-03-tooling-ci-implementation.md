# Strata Tooling & CI/CD Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Set up production-grade Rust tooling, CI/CD pipelines, pre-commit hooks, code coverage, and dependency security scanning for Strata.

**Architecture:** Config files at repo root (rustfmt, clippy, deny, toolchain, bacon), workspace lint policy in Cargo.toml, 5 GitHub Actions workflows, shell-based pre-commit hook, Codecov integration.

**Tech Stack:** Rust stable (rustfmt, clippy, cargo-deny, cargo-llvm-cov), GitHub Actions, Codecov, Node 22, Prettier

**Design Document:** `docs/plans/2026-04-03-tooling-ci-design.md`

---

## Task 1: Rust Tooling Config Files

**Files:**
- Modify: `resource/rustfmt.toml`
- Create: `.clippy.toml`
- Create: `.cargo/config.toml`
- Create: `rust-toolchain.toml`
- Create: `deny.toml`
- Create: `bacon.toml`

**Step 1: Update rustfmt.toml**

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
```

**Step 2: Create `.clippy.toml`**

```toml
msrv = "1.83.0"
disallowed-names = ["foo", "bar", "baz", "tmp", "temp"]
too-many-arguments-threshold = 10
```

**Step 3: Create `.cargo/config.toml`**

```toml
[alias]
check-all = "check --workspace --all-targets"
test-all = "test --workspace"
lint = "clippy --workspace --all-targets -- -D warnings"
```

**Step 4: Create `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "llvm-tools-preview"]
```

**Step 5: Create `deny.toml`**

```toml
[graph]
all-features = true

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]

[licenses]
private = { ignore = true }
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "BSL-1.0",
    "0BSD",
    "Zlib",
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "simplest-path"

[sources]
unknown-registry = "deny"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

**Step 6: Create `bacon.toml`**

```toml
default_job = "check"

[jobs.check]
command = ["cargo", "check", "--workspace", "--color", "always"]
watch = ["resource"]

[jobs.clippy]
command = ["cargo", "clippy", "--workspace", "--all-targets", "--color", "always", "--", "-D", "warnings"]
watch = ["resource"]

[jobs.test]
command = ["cargo", "test", "--workspace", "--color", "always"]
watch = ["resource"]

[jobs.fmt]
command = ["cargo", "fmt", "--all"]
watch = ["resource"]

[keybindings]
c = "job:clippy"
t = "job:test"
f = "job:fmt"
```

**Step 7: Verify toolchain installs**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues (we just formatted)

Run: `cargo clippy --workspace -- -D warnings`
Expected: May have warnings — fix them

**Step 8: Commit**

```bash
git add resource/rustfmt.toml .clippy.toml .cargo/config.toml rust-toolchain.toml deny.toml bacon.toml
git commit -m "chore: add Rust tooling configs (rustfmt, clippy, deny, toolchain, bacon)"
```

---

## Task 2: Workspace Lint Policy & Release Profile

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `resource/Cargo.toml` (add `lints.workspace = true`)

**Step 1: Add lint policy and release profile to workspace `Cargo.toml`**

```toml
[workspace]
members = ["resource"]
resolver = "2"

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

**Step 2: Add `[lints]` section to `resource/Cargo.toml`**

Add at the end of the file:
```toml
[lints]
workspace = true
```

**Step 3: Run clippy to find violations**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: May fail with dead_code/unused_imports errors — fix all of them

**Step 4: Fix all lint violations**

The `Internal` variant in `error/mod.rs` is currently unused. Either use it somewhere or remove it. Since it's a useful variant, add `#[allow(dead_code)]` only if there's a clear future use, otherwise remove it.

Also fix any `unused_imports` violations found.

**Step 5: Verify clean build**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: No warnings, no errors

**Step 6: Commit**

```bash
git add Cargo.toml resource/Cargo.toml resource/core/
git commit -m "chore: add workspace lint policy (forbid unsafe, deny dead_code) and release profile"
```

---

## Task 3: Pre-commit Hook & Setup Script

**Files:**
- Create: `scripts/pre-commit`
- Create: `scripts/setup-hooks.sh`

**Step 1: Create `scripts/pre-commit`**

```sh
#!/bin/sh
# Strata pre-commit hook
# Install: ./scripts/setup-hooks.sh

set -e

echo "Running pre-commit checks..."

# 1. Rust formatting
echo "  [1/3] cargo fmt..."
cargo fmt --all -- --check
if [ $? -ne 0 ]; then
    echo "Formatting check failed. Run: cargo fmt --all"
    exit 1
fi

# 2. Clippy lints
echo "  [2/3] cargo clippy..."
cargo clippy --workspace -- -D warnings 2>/dev/null
if [ $? -ne 0 ]; then
    echo "Clippy found warnings. Fix them before committing."
    exit 1
fi

# 3. Dashboard checks (if dashboard files changed)
DASHBOARD_CHANGED=$(git diff --cached --name-only | grep "^dashboard/" || true)
if [ -n "$DASHBOARD_CHANGED" ]; then
    echo "  [3/3] Prettier + TypeScript check..."
    (cd dashboard && npx prettier --check "src/**/*.{ts,vue}" && npx vue-tsc --noEmit)
    if [ $? -ne 0 ]; then
        echo "Dashboard checks failed."
        exit 1
    fi
else
    echo "  [3/3] Dashboard checks... (skipped, no changes)"
fi

echo "All pre-commit checks passed!"
```

**Step 2: Create `scripts/setup-hooks.sh`**

```sh
#!/bin/sh
# Install Strata git hooks
# Usage: ./scripts/setup-hooks.sh

HOOK_DIR=$(git rev-parse --git-dir)/hooks
cp scripts/pre-commit "$HOOK_DIR/pre-commit"
chmod +x "$HOOK_DIR/pre-commit"
echo "Pre-commit hook installed."
```

**Step 3: Make both executable**

Run: `chmod +x scripts/pre-commit scripts/setup-hooks.sh`

**Step 4: Install hook locally**

Run: `./scripts/setup-hooks.sh`
Expected: "Pre-commit hook installed."

**Step 5: Commit**

```bash
git add scripts/
git commit -m "chore: add pre-commit hook (fmt, clippy, prettier, tsc)"
```

---

## Task 4: Codecov Config

**Files:**
- Create: `codecov.yml`

**Step 1: Create `codecov.yml`**

```yaml
codecov:
  require_ci_to_pass: true

coverage:
  status:
    project:
      default:
        target: auto
        threshold: 5%
    patch:
      default:
        target: 70%

ignore:
  - "resource/migrations/"
  - "dashboard/"
  - "dev/"
  - "scripts/"
  - "docs/"
```

**Step 2: Commit**

```bash
git add codecov.yml
git commit -m "chore: add Codecov config (5% project threshold, 70% patch)"
```

---

## Task 5: CI Workflow — Main Pipeline

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on: [push, pull_request]

jobs:
  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt }
      - run: cargo fmt --all -- --check

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16-alpine
        env: { POSTGRES_DB: strata_test, POSTGRES_USER: test, POSTGRES_PASSWORD: test }
        ports: [5432:5432]
        options: --health-cmd "pg_isready -U test" --health-interval 5s --health-retries 5
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: llvm-tools-preview }
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-llvm-cov
      - run: cargo llvm-cov --workspace --lcov --output-path lcov.info
        env:
          DATABASE_URL: postgres://test:test@localhost:5432/strata_test
      - uses: codecov/codecov-action@v4
        with:
          token: ${{ secrets.CODECOV_TOKEN }}
          files: lcov.info

  dashboard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '22' }
      - run: cd dashboard && npm ci && npm run build
```

**Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add main pipeline (fmt, clippy, test + coverage, dashboard build)"
```

---

## Task 6: Security Workflow

**Files:**
- Create: `.github/workflows/security.yml`

**Step 1: Create `.github/workflows/security.yml`**

```yaml
name: Security

on: [push, pull_request]

jobs:
  cargo-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install cargo-audit
      - run: cargo audit

  cargo-deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
```

**Step 2: Commit**

```bash
git add .github/workflows/security.yml
git commit -m "ci: add security workflow (cargo-audit, cargo-deny)"
```

---

## Task 7: Claude Code PR Review Workflow

**Files:**
- Create: `.github/workflows/claude.yml`

**Step 1: Create `.github/workflows/claude.yml`**

```yaml
name: Claude Analysis

on:
  pull_request:
    types: [opened, synchronize, reopened]
  issue_comment:
    types: [created]

jobs:
  analyze:
    if: |
      github.event_name == 'pull_request' ||
      (github.event_name == 'issue_comment' &&
       github.event.issue.pull_request &&
       contains(github.event.comment.body, '@claude'))
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write
      contents: read
      issues: read
      id-token: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: anthropics/claude-code-action@v1
        with:
          claude_code_oauth_token: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}
          prompt: |
            You are reviewing a PR for Strata — an open-source observability dashboard
            (Grafana alternative) with a Rust/Axum backend and Vue 3 frontend.

            Analyze this PR and provide a structured review as a PR comment:

            ## Strata PR Analysis

            ### Critical (must fix before merge)
            - Security issues (SQL injection, credential leaks, XSS)
            - Breaking API changes
            - Data exposure in logs or responses

            ### Warnings (should fix)
            - Performance regressions (N+1 queries, missing indexes)
            - Missing error handling
            - Dead code or unused imports

            ### Positive
            - Good patterns worth noting

            ### Summary
            | Check | Status |
            |-------|--------|
            | Security | pass/warn/fail |
            | Tests | pass/warn/fail |
            | Performance | pass/warn/fail |
            | Code Quality | pass/warn/fail |

            Be concise. Only flag real issues, not style preferences.
```

**Step 2: Commit**

```bash
git add .github/workflows/claude.yml
git commit -m "ci: add Claude Code PR review workflow"
```

---

## Task 8: Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Verify tag is on main branch
        run: |
          TAG="${GITHUB_REF_NAME}"
          COMMIT=$(git rev-list -n 1 "$TAG")
          if ! git merge-base --is-ancestor "$COMMIT" origin/main; then
            echo "Tag $TAG is not on main branch"
            exit 1
          fi

  build:
    needs: validate
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - uses: actions/checkout@v4

      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: ghcr.io/${{ github.repository }}:${{ github.ref_name }},ghcr.io/${{ github.repository }}:latest
```

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow (Docker build + ghcr.io push on tag)"
```

---

## Task 9: Update CLAUDE.md with Tooling Commands

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add tooling section to CLAUDE.md**

Add after the existing Build Commands section:

```markdown
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
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with tooling and lint policy"
```

---

## Task 10: Final Verification & Push

**Step 1: Run full quality check**

Run: `cargo fmt --all -- --check`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 2: Push and verify CI triggers**

Run: `git push`
Expected: GitHub Actions workflows trigger on push
