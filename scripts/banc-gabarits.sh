#!/usr/bin/env bash
# The generated-template bench, end to end: compose the three reference sets
# on the six page formats (with the two alternative proposals beside each
# album), then let the linter judge every enumerated combination by
# substitution (`--banc-gabarits`). The verdict lands in
# .albums/banc/rapport.json; its `verts` array is what `gabarit::RETENUS`
# declares, pasted by hand so the review reads the decision.
#
# Fresh directories every run: album.origin.json is never rewritten, and a
# bench measuring today's candidates against last month's composition would
# lie quietly.
set -euo pipefail
cd "$(dirname "$0")/.."

TESTSETS="$HOME/Pictures/colophon-testsets"
[ -d "$TESTSETS" ] || { echo "jeux de test absents : $TESTSETS" >&2; exit 1; }

cargo build --release

FORMATS="carre-21 carre-30 portrait-a4 paysage-a4 paysage-28x21 portrait-20x25"
DIRS=()
for set in corse-2013 mauritanie-2019 random-2024; do
  [ -d "$TESTSETS/$set" ] || { echo "jeu absent : $set" >&2; exit 1; }
  for format in $FORMATS; do
    out=".albums/banc/$set-$format"
    rm -rf "$out"
    echo "compose : $set en $format"
    ./target/release/colophon "$TESTSETS/$set" -o "$out" --format "$format" \
      --variantes >/dev/null 2>&1
    DIRS+=("$out")
  done
done

./target/release/colophon --banc-gabarits "${DIRS[@]}" > .albums/banc/rapport.json
python3 - <<'EOF'
import json
r = json.load(open('.albums/banc/rapport.json'))
print(f"{r['candidats']} candidats, {r['albums']} albums, jeux : {len(r['jeux'])}")
print(f"verts : {len(r['verts'])}, recalés : {r['recales']}, "
      f"sans essai : {r['sans_essai']}, jeu manquant : {r['jeu_manquant']}")
for v in r['verts']:
    print(f'    "{v}",')
EOF
echo "rapport : .albums/banc/rapport.json"
