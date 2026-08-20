#!/usr/bin/env bash
# Regenerate the committed scene fixture.
#
# Its input is `scene.album.json`, hand-written to hold one of everything a
# spread can be — the half-title, a full-bleed page, an empty caption that must
# produce no object, a verso truncated below its capacity, a declared caption
# band, a text page with a blank line in it, a template nobody knows, the
# colophon. A fixture taken from a real album would cover whatever that album
# happened to contain, and would need photographs nobody else has.
#
# The same committed album is re-derived at each page shape, which is how one
# file covers the six formats without being composed six times.
#
# Run this whenever the scene changes shape. `parity.test.ts` fails until you
# do, which is the point: a stale fixture is a test of last month's model.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=./target/release/colophon
[ -x "$BIN" ] || { echo "construisez d'abord : cargo build --release" >&2; exit 1; }

ALBUM=crates/colophon-app/src/scene.album.json
OUT=crates/colophon-app/src/scene.fixture.json

python3 - "$BIN" "$ALBUM" "$OUT" <<'PY'
import json, subprocess, sys, pathlib

# The page shapes the raster gate walks, in the same order.
FORMATS = [
    "carre-21",
    "carre-30",
    "portrait-a4",
    "paysage-a4",
    "paysage-28x21",
    "portrait-20x25",
]
binaire, album, sortie = sys.argv[1:4]
out = {
    f: json.loads(
        subprocess.check_output([binaire, "--dump-scene", album, "--format", f], text=True)
    )
    for f in FORMATS
}
p = pathlib.Path(sortie)
p.write_text(json.dumps(out, indent=1, ensure_ascii=False, sort_keys=True) + "\n")
print(f"{p} : {p.stat().st_size / 1024:.1f} Kio, {len(out)} formats")
PY
