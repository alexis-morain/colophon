#!/usr/bin/env bash
# The whole gate in one command: what CI runs, runnable locally.
# Order matters: the release binary must exist before the Vitest parity
# test, which executes it (and skips silently when it is missing).
set -euo pipefail
cd "$(dirname "$0")/.."

# The gate reads its own reports with python. python3 everywhere, except the
# Windows runner, where the interpreter only answers to python — resolved
# once, refused loudly when neither name exists.
PY="$(command -v python3 || command -v python || true)"
if [ -z "$PY" ]; then
  echo "python introuvable : le gate s'en sert pour lire ses rapports" >&2
  exit 1
fi

cargo build --release
cargo test --workspace

# Album linter as a non-regression gate: recompose the reference sets with
# today's code, then audit. A Composer change that raises a counter fails
# here before it ships. It never skips: with the photos it composes from
# them, without them it composes from their fiches, and where both are at
# hand it asserts the two give the same album to the byte.
TESTSETS="${COLOPHON_TESTSETS:-$HOME/Pictures/colophon-testsets}"
FICHES="crates/colophon-core/fiches"
for set in corse-2013 mauritanie-2019 random-2024; do
  # No fiches, no linter, and that is a failure rather than a tolerance: the
  # album linter is the Composer's non-regression gate, and a gate that
  # steps aside is not a gate.
  if [ ! -f "$FICHES/$set.json" ]; then
    echo "fiches manquantes pour $set : ./scripts/fiches.sh les régénère" >&2
    exit 1
  fi
  out=".albums/check/$set"
  temoin=""
  # From scratch, every time. album.origin.json is never rewritten, being
  # the reprise's reference: composing into a folder an earlier run left
  # behind measures today's album against last month's proposal, and the
  # reprise then reads a hand correction nobody ever made.
  rm -rf "$out"
  if [ -d "$TESTSETS/$set" ]; then
    ./target/release/colophon "$TESTSETS/$set" -o "$out" --format carre-21 \
      --variantes >/dev/null 2>&1
    # The same album again, composed from the versioned fiches. One assertion
    # doing two jobs: it is this feature's own test, and it is the freshness
    # test of the fiches, exactly as the scene fixture is for the geometry
    # dump. Both compositions run in the same minute, so the colophon's
    # « composé le » date is the same on both sides.
    temoin=".albums/check/$set-fiches"
    rm -rf "$temoin"
    ./target/release/colophon --depuis-fiches "$FICHES/$set.json" -o "$temoin" \
      --format carre-21 --variantes >/dev/null 2>&1
    "$PY" scripts/identite-fiches.py "$out" "$temoin" "$set"
  else
    ./target/release/colophon --depuis-fiches "$FICHES/$set.json" -o "$out" \
      --format carre-21 --variantes >/dev/null 2>&1
    echo "linter $set : composé depuis les fiches, les photos n'étant pas là"
  fi
  if ! ./target/release/colophon --audit -o "$out" > "$out/audit.json"; then
    echo "audit : compteurs au-dessus des seuils sur $set" >&2
    "$PY" -c "
import json
r = json.load(open('$out/audit.json'))
for k, c in r['compteurs'].items():
    if c['count'] > c['seuil']:
        print(f\"  {k}: {c['count']} (seuil {c['seuil']})\")" >&2
    exit 1
  fi
  # What the linter measured, and on what. An audit that starts from the
  # fiches says so in its report; relaying it here is the whole difference
  # between a green and a green somebody can weigh.
  "$PY" -c "
import json
r = json.load(open('$out/audit.json'))
for n in r.get('notes', []):
    print(f'  {n}')"
  # Same album is not the same verdict, and the verdict is what this gate
  # exists for. A fiche can go stale without moving a single spread and still
  # move a counter — colorsig feeds the doublon rule, which no spread reads —
  # so the two linters are compared, not only the two albums they read.
  if [ -n "$temoin" ]; then
    # || true : a counter may cross its threshold on the fiches side only,
    # and that story belongs to the comparison below, not to a silent set -e.
    ./target/release/colophon --audit -o "$temoin" > "$temoin/audit.json" || true
    "$PY" -c "
import json, sys
a = json.load(open('$out/audit.json'))['compteurs']
b = json.load(open('$temoin/audit.json'))['compteurs']
ecarts = [f\"  {k} : {a[k]['count']} depuis les photos, {b[k]['count']} depuis les fiches\"
          for k in a if a[k]['count'] != b[k]['count']]
if ecarts:
    print('verdict $set : le linter ne dit pas la même chose des deux côtés', file=sys.stderr)
    print(chr(10).join(ecarts), file=sys.stderr)
    print('  ./scripts/fiches.sh', file=sys.stderr)
    sys.exit(1)"
    echo "verdict $set : mêmes compteurs depuis les photos et depuis les fiches"
  fi
  # A freshly composed album has been corrected by nobody, so the reprise
  # must read exactly zero. It fails here if the composer stopped laying
  # down its reference, or if the diff started seeing corrections in an
  # untouched book.
  if ! ./target/release/colophon --reprise -o "$out" > "$out/reprise.json"; then
    echo "reprise : l'album neuf de $set ne mesure pas zéro" >&2
    exit 1
  fi
  "$PY" -c "
import json, sys
r = json.load(open('$out/reprise.json'))
if r['planches_touchees'] != 0:
    print(f\"  reprise {r['planches_touchees']} planches sur un album neuf\", file=sys.stderr)
    sys.exit(1)"
  # The two proposals shown beside the album go through the same linter:
  # an option the composer offers is an album somebody will print, and a
  # variant green nowhere is a variant that must not be offered.
  for v in autre-rythme resserree; do
    cp "$out/album.$v.json" "$out/album.json"
    if ! ./target/release/colophon --audit -o "$out" > "$out/audit-$v.json"; then
      echo "audit : la variante $v de $set passe un seuil" >&2
      "$PY" -c "
import json
r = json.load(open('$out/audit-$v.json'))
for k, c in r['compteurs'].items():
    if c['count'] > c['seuil']:
        print(f\"  {k}: {c['count']} (seuil {c['seuil']})\")" >&2
      exit 1
    fi
  done
  cp "$out/album.demandee.json" "$out/album.json"
  echo "audit $set : ok (3 propositions)"
done

# PDF → PNG : le rendu réel de chaque gabarit, rasterisé et vérifié case
# par case sur les six formats. macOS seulement (sips) et Pillow requis ;
# ailleurs le test passe son tour en le disant.
if command -v sips >/dev/null && "$PY" -c "import PIL" 2>/dev/null; then
  "$PY" scripts/pdf-png.py
else
  echo "pdf-png : sauté (sips ou Pillow absent)"
fi

cd crates/colophon-app
npx tsc --noEmit
npx vitest run
