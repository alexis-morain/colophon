#!/usr/bin/env bash
# The whole gate in one command: what CI runs, runnable locally.
# Order matters: the release binary must exist before the Vitest parity
# test, which executes it (and skips silently when it is missing).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
cargo test --workspace

# Album linter as a non-regression gate: recompose the reference sets with
# today's code, then audit. A Composer change that raises a counter fails
# here before it ships. Skips silently on machines without the test sets.
TESTSETS="$HOME/Pictures/colophon-testsets"
if [ -d "$TESTSETS" ]; then
  for set in corse-2013 mauritanie-2019 random-2024; do
    [ -d "$TESTSETS/$set" ] || continue
    out=".albums/check/$set"
    ./target/release/colophon "$TESTSETS/$set" -o "$out" --format carre-21 \
      >/dev/null 2>&1
    if ! ./target/release/colophon --audit -o "$out" > "$out/audit.json"; then
      echo "audit : compteurs au-dessus des seuils sur $set" >&2
      python3 -c "
import json
r = json.load(open('$out/audit.json'))
for k, c in r['compteurs'].items():
    if c['count'] > c['seuil']:
        print(f\"  {k}: {c['count']} (seuil {c['seuil']})\")" >&2
      exit 1
    fi
    echo "audit $set : ok"
  done
fi

cd crates/colophon-app
npx tsc --noEmit
npx vitest run
