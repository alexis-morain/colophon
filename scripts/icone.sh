#!/usr/bin/env bash
# Une marque en SVG, tout le reste en une commande.
#
# L'icône de l'app était encore celle de Tauri : dans le Dock, dans le DMG,
# et donc dans chaque capture d'écran qui circulera. Le fond est un
# arbitrage, la mécanique n'en est pas un : ce script produit l'.icns,
# l'.ico, les PNG du bundle, le favicon du site et l'image sociale depuis
# un fichier source unique.
#
#   scripts/icone.sh design/marques/coupe.svg
#
# Changer de marque, une fois le choix fait, coûte exactement cette ligne.
# Prérequis : rsvg-convert (brew install librsvg) et npm install fait.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC="${1:-design/marques/coupe.svg}"
[ -f "$SRC" ] || { echo "marque introuvable : $SRC" >&2; exit 2; }
command -v rsvg-convert >/dev/null || {
  echo "rsvg-convert absent : brew install librsvg" >&2; exit 2; }

OUT="design/rendu"
mkdir -p "$OUT"

# 1024 px : la source que `tauri icon` décline en .icns, .ico et en tous les
# PNG du bundle, Windows compris. Une seule taille à produire ici.
rsvg-convert -w 1024 -h 1024 "$SRC" -o "$OUT/icone-1024.png"

# Le favicon du site : 32 px, la taille où la marque doit encore tenir.
rsvg-convert -w 32 -h 32 "$SRC" -o "$OUT/favicon-32.png"
rsvg-convert -w 180 -h 180 "$SRC" -o "$OUT/apple-touch-icon.png"

# L'image sociale, 1200 × 630 : la marque centrée sur le papier, pas
# étirée. rsvg-convert ne recadre pas, donc la marque est rendue au carré
# puis posée sur un fond à la bonne forme.
rsvg-convert -w 630 -h 630 "$SRC" -o "$OUT/social-carre.png"
python3 - "$OUT" <<'PY'
import sys
from pathlib import Path
try:
    from PIL import Image
except ImportError:
    print("Pillow absent : image sociale sautée", file=sys.stderr)
    raise SystemExit(0)
out = Path(sys.argv[1])
marque = Image.open(out / "social-carre.png").convert("RGB")
fond = Image.new("RGB", (1200, 630), (246, 242, 233))
fond.paste(marque, ((1200 - 630) // 2, 0))
fond.save(out / "social-1200x630.png")
print(f"  image sociale : {out / 'social-1200x630.png'}")
PY

# Le jeu du bundle, par l'outil de Tauri : il écrit dans src-tauri/icons,
# exactement les fichiers que tauri.conf.json déclare.
(cd crates/colophon-app && npx tauri icon "../../$OUT/icone-1024.png")
# `tauri icon` écrit aussi les jeux Android et iOS. Colophon est un logiciel
# de bureau : ces dossiers ne partent nulle part et n'ont rien à faire dans
# le dépôt.
rm -rf crates/colophon-app/src-tauri/icons/android crates/colophon-app/src-tauri/icons/ios

echo "icône : $SRC → crates/colophon-app/src-tauri/icons et $OUT"
