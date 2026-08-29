#!/bin/sh
# Bundle Colophon.app puis remplace la copie de /Applications.
# Une seule version installée à la fois : l'ancienne est supprimée, jamais
# renommée en « Colophon 2.app » par un glisser-déposer de plus.
set -e

cd "$(dirname "$0")/../crates/colophon-app"
npm run tauri build

APP="../../target/release/bundle/macos/Colophon.app"
DEST="/Applications/Colophon.app"

# Le processus s'appelle « colophon-app », pas « Colophon » : c'est le
# CFBundleExecutable, et c'est ce que pgrep voit. Écrite avec le nom
# d'affichage, la garde ne matchait jamais — le bundle se faisait remplacer
# sous une app qui tournait, et l'app gardait l'ancien code. Trouvé le 29/08
# en vérifiant un correctif à l'écran : c'est l'ancien qui répondait.
if pgrep -x colophon-app >/dev/null 2>&1; then
  echo "Colophon tourne : quitter l'app avant de la remplacer." >&2
  exit 1
fi


rm -rf "$DEST"
ditto "$APP" "$DEST"
echo "Installé : $DEST ($(plutil -extract CFBundleShortVersionString raw "$DEST/Contents/Info.plist"))"
