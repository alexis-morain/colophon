#!/bin/bash
# La version Mac App Store : la même app, compilée autrement.
#
#   mas.sh build              Colophon.app universel, bac à sable, sans updater
#   mas.sh build --essai      la même, signée ad hoc même si les identités existent
#   mas.sh pkg                le .pkg signé « 3rd Party Mac Developer Installer »
#   mas.sh valider            xcrun altool --validate-app sur le .pkg
#   mas.sh envoyer --oui      xcrun altool --upload-app, geste d'Alexis
#
# Les identités et la clé d'API se lisent dans le .env de gestion
# (`APPLE_MAS_APP`, `APPLE_MAS_INSTALLER`, `APPLE_MAS_API_KEY_ID`,
# `APPLE_MAS_API_ISSUER`, `COLOPHON_MAS_PROFIL`), ou dans l'environnement qui
# les a déjà, et ne s'affichent jamais. Sans identité ni profil, `build`
# produit quand même une app **signée ad hoc** sous bac à sable pour l'essai
# local, et le dit : les deux identifiants (`application-identifier`,
# `team-identifier`) sont alors retirés de la signature, que macOS refuserait
# sans profil embarqué. Une telle app ne part jamais chez Apple.
#
# À l'inverse, une app signée « Apple Distribution » ne se lance pas sur ce Mac
# (AMFI : « No matching profile found », mesuré le 16/10) : un profil App Store
# ne sert qu'au Store. Pour l'essai à l'écran, `build --essai` signe ad hoc
# même quand le .env porte les identités.
#
# Ce que `build` prouve sur ce qu'il vient de produire, au lieu de le supposer :
# le binaire ne porte ni le flux des versions ni la clé publique de l'updater,
# le bundle web ne porte pas le plugin updater, et la signature porte le bac à
# sable à vrai.
#
# Le DMG et la CI publique ne passent jamais par ici.
set -euo pipefail
cd "$(dirname "$0")/.."

APP_DIR="crates/colophon-app"
TAURI="$APP_DIR/src-tauri"
CIBLE="universal-apple-darwin"
SORTIE="target/mas"
BUNDLE="target/$CIBLE/release/bundle/macos/Colophon.app"
PKG="$SORTIE/Colophon.pkg"
GESTION="${COLOPHON_GESTION:-$HOME/Documents/08_IA/02_Outputs/colophon}"

echouer() { echo "mas : $*" >&2; exit 1; }

# Une valeur du .env de gestion, sans jamais l'afficher. L'environnement
# l'emporte : c'est ce qui permet d'essayer une autre identité sans éditer le
# fichier.
lire() {
  local nom="$1" val="${!1:-}"
  if [ -z "$val" ] && [ -f "$GESTION/.env" ]; then
    val="$(grep -E "^${nom}=" "$GESTION/.env" | tail -1 | cut -d= -f2- || true)"
    val="${val%\"}"; val="${val#\"}"
  fi
  printf '%s' "$val"
}

pas_de_construction_en_cours() {
  if pgrep -f "tauri build|bundle_dmg.sh" >/dev/null; then
    echouer "une construction tourne déjà (tauri build ou bundle_dmg.sh) : attendre qu'elle finisse"
  fi
}

build() {
  local essai=0
  case "${1:-}" in
    "") ;;
    --essai) essai=1 ;;
    *) echouer "option inconnue : $1 (seule --essai existe)" ;;
  esac
  pas_de_construction_en_cours
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target list --installed | grep -qx "$t" \
      || echouer "cible Rust absente : rustup target add $t"
  done
  rm -rf "$SORTIE"
  mkdir -p "$SORTIE"

  local identite profil conf
  identite="$(lire APPLE_MAS_APP)"
  profil="$(lire COLOPHON_MAS_PROFIL)"
  conf="$TAURI/tauri.appstore.conf.json"

  if [ "$essai" = 1 ]; then
    identite=""; profil=""
  fi
  if [ -n "$identite" ] && [ -n "$profil" ]; then
    [ -f "$profil" ] || echouer "COLOPHON_MAS_PROFIL ne désigne aucun fichier"
    # Le profil le temps de la construction, jamais suivi par git (.gitignore).
    cp "$profil" "$TAURI/embedded.provisionprofile"
    trap 'rm -f "$TAURI/embedded.provisionprofile"' EXIT
    echo "mas : signature App Store, profil embarqué"
  else
    identite="-"
    if [ "$essai" = 1 ]; then
      echo "mas : --essai : signature AD HOC, pour l'essai local seulement"
    else
      echo "mas : AUCUNE identité App Store ou aucun profil : signature AD HOC, pour l'essai local seulement"
    fi
    # La même configuration, sans profil et avec des droits sans les deux
    # identifiants. Écrite dans target/, chemins absolus.
    python3 - "$conf" "$PWD/$TAURI" "$PWD/$SORTIE" <<'PY'
import json, plistlib, sys
conf, tauri, sortie = sys.argv[1:]
c = json.load(open(conf))
mac = c["bundle"]["macOS"]
mac["files"].pop("embedded.provisionprofile")
mac["files"] = {k: f"{tauri}/{v}" for k, v in mac["files"].items()}
mac["infoPlist"] = f"{tauri}/{mac['infoPlist']}"
d = plistlib.load(open(f"{tauri}/{mac['entitlements']}", "rb"))
for cle in ("com.apple.application-identifier", "com.apple.developer.team-identifier"):
    d.pop(cle)
plistlib.dump(d, open(f"{sortie}/entitlements.essai.plist", "wb"))
mac["entitlements"] = f"{sortie}/entitlements.essai.plist"
json.dump(c, open(f"{sortie}/tauri.essai.conf.json", "w"), indent=2)
PY
    conf="$SORTIE/tauri.essai.conf.json"
  fi

  # Aucune notarisation ici : le Store ne la demande pas, et Tauri la tente
  # dès que ces variables existent.
  (
    cd "$APP_DIR"
    env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID -u APPLE_API_KEY -u APPLE_API_ISSUER \
      -u APPLE_CERTIFICATE -u APPLE_CERTIFICATE_PASSWORD \
      APPLE_SIGNING_IDENTITY="$identite" VITE_COLOPHON_MAS=1 \
      npm run tauri build -- --target "$CIBLE" --bundles app \
      --config "$OLDPWD/$conf" --features mas
  )
  [ -d "$BUNDLE" ] || echouer "pas d'app à $BUNDLE"

  # L'attribut de quarantaine est refusé par le Store depuis le 18/02/2025.
  xattr -cr "$BUNDLE"
  prouver "$identite"
  ditto "$BUNDLE" "$SORTIE/Colophon.app"
  echo "mas : $SORTIE/Colophon.app"
}

# Les preuves, sur les fichiers produits.
prouver() {
  local identite="$1" binaire="$BUNDLE/Contents/MacOS/colophon-app"
  [ -f "$binaire" ] || binaire="$(find "$BUNDLE/Contents/MacOS" -type f -perm -u+x | head -1)"
  lipo -archs "$binaire" | grep -q "x86_64" && lipo -archs "$binaire" | grep -q "arm64" \
    || echouer "le binaire n'est pas universel : $(lipo -archs "$binaire")"
  local cle
  cle="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["plugins"]["updater"]["pubkey"])' "$TAURI/tauri.conf.json")"
  if grep -aqF "latest.json" "$binaire"; then echouer "le binaire porte encore le flux des versions"; fi
  if grep -aqF "${cle:0:40}" "$binaire"; then echouer "le binaire porte encore la clé de l'updater"; fi
  if grep -rqF "plugin:updater" "$APP_DIR/dist"; then echouer "le bundle web porte encore le plugin updater"; fi
  codesign --verify --strict "$BUNDLE" || echouer "signature invalide"
  codesign -d --entitlements - --xml "$BUNDLE" 2>/dev/null | python3 -c '
import plistlib, sys
d = plistlib.loads(sys.stdin.buffer.read())
sys.exit(0 if d.get("com.apple.security.app-sandbox") is True else 1)
' || echouer "la signature ne porte pas app-sandbox à vrai"
  [ -f "$BUNDLE/Contents/Resources/PrivacyInfo.xcprivacy" ] || echouer "PrivacyInfo.xcprivacy absent du bundle"
  if [ "$identite" != "-" ]; then
    [ -f "$BUNDLE/Contents/embedded.provisionprofile" ] || echouer "profil absent du bundle"
  fi
  echo "mas : universel, sans updater, bac à sable à vrai, manifeste de confidentialité présent"
}

pkg() {
  local installeur
  installeur="$(lire APPLE_MAS_INSTALLER)"
  [ -n "$installeur" ] || echouer "APPLE_MAS_INSTALLER manque : pas de .pkg sans l'identité « 3rd Party Mac Developer Installer »"
  [ -d "$SORTIE/Colophon.app" ] || echouer "pas d'app : mas.sh build d'abord"
  [ -f "$SORTIE/Colophon.app/Contents/embedded.provisionprofile" ] \
    || echouer "app signée ad hoc : elle ne part pas chez Apple, refaire build avec les identités"
  productbuild --component "$SORTIE/Colophon.app" /Applications --sign "$installeur" "$PKG"
  echo "mas : $PKG"
}

# altool lit la clé .p8 dans ~/.appstoreconnect/private_keys/ ; on ne la passe
# jamais en argument.
altool() {
  local action="$1" id emetteur
  id="$(lire APPLE_MAS_API_KEY_ID)"; emetteur="$(lire APPLE_MAS_API_ISSUER)"
  [ -n "$id" ] && [ -n "$emetteur" ] || echouer "APPLE_MAS_API_KEY_ID et APPLE_MAS_API_ISSUER manquent"
  [ -f "$PKG" ] || echouer "pas de .pkg : mas.sh pkg d'abord"
  xcrun altool "$action" --type macos -f "$PKG" --apiKey "$id" --apiIssuer "$emetteur"
}

case "${1:-}" in
  build) build "${2:-}" ;;
  pkg) pkg ;;
  valider) altool --validate-app ;;
  envoyer)
    [ "${2:-}" = "--oui" ] || echouer "l'envoi est le geste d'Alexis : mas.sh envoyer --oui"
    altool --upload-app ;;
  *) echo "usage : mas.sh build [--essai]|pkg|valider|envoyer --oui" >&2; exit 2 ;;
esac
