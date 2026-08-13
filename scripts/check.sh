#!/usr/bin/env bash
# The whole gate in one command: what CI runs, runnable locally.
# Order matters: the release binary must exist before the Vitest parity
# test, which executes it (and skips silently when it is missing).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
cargo test --workspace

cd crates/colophon-app
npx tsc --noEmit
npx vitest run
