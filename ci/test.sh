#!/usr/bin/env bash
set -euo pipefail

# Clippy targets wasm32 since alkahest-web uses web-only APIs
# Exclude alkahest-bench (native-only: uses pollster, env_logger)
cargo clippy --workspace --exclude alkahest-bench --target wasm32-unknown-unknown -- -D warnings

# Clippy for alkahest-bench on native target
cargo clippy -p alkahest-bench -- -D warnings

# Format check is target-agnostic
cargo fmt --all -- --check

# Unit tests run on host for alkahest-core (pure Rust, no web deps)
cargo test -p alkahest-core
