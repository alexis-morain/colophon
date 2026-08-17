#!/usr/bin/env bash
# Third-party notices, from the two lock files to one embedded Markdown file.
#
# GPL-3.0 is a copyleft licence, and the crates and packages under it travel
# in the binary: the terms of the ones that ask for it (MIT, BSD, Apache-2.0,
# ISC…) have to travel with them. This writes the list the About screen shows
# and the repo publishes.
#
# Rust: cargo-about, driven by about.toml, over 492 crates.
# JavaScript: license-checker-rseidelsohn if it is there; otherwise the
# package-lock is read directly, which gives names, versions and SPDX ids but
# not the licence texts. The script says which of the two it did.
set -euo pipefail
cd "$(dirname "$0")/.."

SORTIE="crates/colophon-app/src-tauri/notices.md"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

{
  echo "# Notices des licences tierces"
  echo
  echo "Colophon est distribué sous GPL-3.0. Il embarque les bibliothèques"
  echo "ci-dessous, chacune sous ses propres termes, reproduits ici comme"
  echo "elles l'exigent."
  echo
  echo "Régénéré par \`scripts/notices.sh\`."
  echo
} > "$tmp"

if command -v cargo-about >/dev/null; then
  echo "## Rust (Cargo.lock)" >> "$tmp"
  echo >> "$tmp"
  # Colophon's own three crates are not a third party to itself: they are
  # filtered out here rather than in about.toml, whose `private` key only
  # covers unpublished crates by registry, not by name.
  cargo about generate about.hbs \
    | python3 -c "
import re, sys
texte = sys.stdin.read()
blocs = re.split(r'^### ', texte, flags=re.M)
garde = [blocs[0]]
for b in blocs[1:]:
    if 'GNU General Public License v3.0' in b.split(chr(10))[0] and 'colophon-core' in b:
        continue
    garde.append('### ' + b)
sys.stdout.write(''.join(garde))
" >> "$tmp"
else
  echo "cargo-about absent : les notices Rust ne peuvent pas être générées" >&2
  echo "  cargo install cargo-about --locked --features cli" >&2
  exit 1
fi

{
  echo
  echo "## JavaScript (package-lock.json)"
  echo
} >> "$tmp"

python3 - "$tmp" <<'PY'
import json
import sys
from pathlib import Path

sortie = Path(sys.argv[1])
lock = json.loads(Path("crates/colophon-app/package-lock.json").read_text())
paquets = {}
for chemin, info in lock.get("packages", {}).items():
    if not chemin.startswith("node_modules/"):
        continue
    nom = chemin.split("node_modules/")[-1]
    licence = info.get("license") or info.get("licenses") or "non déclarée"
    if isinstance(licence, list):
        licence = ", ".join(str(x) for x in licence)
    paquets[nom] = (info.get("version", "?"), str(licence))

lignes = [f"- **{n}** {v} — {l}" for n, (v, l) in sorted(paquets.items())]
with sortie.open("a", encoding="utf-8") as f:
    f.write(f"{len(lignes)} paquets.\n\n")
    f.write("\n".join(lignes))
    f.write("\n")
PY

mkdir -p "$(dirname "$SORTIE")"
mv "$tmp" "$SORTIE"
trap - EXIT
# The repo publishes the same file: a notice only the binary carries is a
# notice nobody can check before installing.
cp "$SORTIE" NOTICES.md
echo "notices : $(wc -l < "$SORTIE") lignes dans $SORTIE et NOTICES.md"
