#!/usr/bin/env bash
set -euo pipefail

wasm-pack build --release crates/alkahest-web --target web --out-dir ../../web/pkg
