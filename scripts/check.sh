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
    # A freshly composed album has been corrected by nobody, so the reprise
    # must read exactly zero. It fails here if the composer stopped laying
    # down its reference, or if the diff started seeing corrections in an
    # untouched book.
    if ! ./target/release/colophon --reprise -o "$out" > "$out/reprise.json"; then
      echo "reprise : l'album neuf de $set ne mesure pas zéro" >&2
      exit 1
    fi
    python3 -c "
import json, sys
r = json.load(open('$out/reprise.json'))
if r['planches_touchees'] != 0:
    print(f\"  reprise {r['planches_touchees']} planches sur un album neuf\", file=sys.stderr)
    sys.exit(1)"
    echo "audit $set : ok"
  done
fi

# PDF → PNG : le rendu réel de chaque gabarit, rasterisé et vérifié case
# par case sur les six formats. macOS seulement (sips) et Pillow requis ;
# ailleurs le test passe son tour en le disant.
if command -v sips >/dev/null && python3 -c "import PIL" 2>/dev/null; then
  python3 scripts/pdf-png.py
else
  echo "pdf-png : sauté (sips ou Pillow absent)"
fi

cd crates/colophon-app
npx tsc --noEmit
npx vitest run
