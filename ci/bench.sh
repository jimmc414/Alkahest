#!/usr/bin/env bash
set -euo pipefail

cargo run -p alkahest-bench --release -- \
    --baseline tests/benchmarks/baselines/latest.json \
    --output tests/benchmarks/baselines/current.json \
    --regression-threshold 10
