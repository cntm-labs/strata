#!/bin/sh
# Install Strata git hooks
# Usage: ./scripts/setup-hooks.sh

HOOK_DIR=$(git rev-parse --git-dir)/hooks
cp scripts/pre-commit "$HOOK_DIR/pre-commit"
chmod +x "$HOOK_DIR/pre-commit"
echo "Pre-commit hook installed."
