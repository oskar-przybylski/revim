#!/usr/bin/env bash
# Runs the same checks as .github/workflows/ci.yml.
# Keep both in sync when adding or changing a CI step.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

echo "==> cargo fmt --all --check"
cargo fmt --all --check

echo "==> cargo clippy --workspace --all-targets --locked -- -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> cargo test --workspace --locked"
cargo test --workspace --locked

echo "==> all checks passed"
