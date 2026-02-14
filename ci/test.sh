#!/usr/bin/env bash
set -euo pipefail

# Clippy targets wasm32 since alkahest-web uses web-only APIs
cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings

# Format check is target-agnostic
cargo fmt --all -- --check

# Unit tests run on host for alkahest-core (pure Rust, no web deps)
cargo test -p alkahest-core
