#!/bin/bash
# La signature Developer ID et la notarisation, vérifiées et non supposées.
#
# Tauri signe l'app, la notarise et l'agrafe, puis signe le DMG. Deux choses
# lui échappent, et ce script les fait : il ne notarise pas le DMG, et il
# **saute la notarisation avec un simple avertissement** quand un identifiant
# manque. Un bundle signé mais non notarisé est bloqué par Gatekeeper comme un
# bundle nu, et le journal de build ne le dirait qu'en jaune.
#
#   signature-macos.sh verifier   <Colophon.app> [--distribution]
#   signature-macos.sh notariser  <Colophon.dmg>
#   signature-macos.sh gatekeeper <Colophon.app> <Colophon.dmg>
#
# `verifier` tourne aussi sur la machine de dev, avec un certificat Apple
# Development : il prouve que le runtime durci et la clé de la photothèque
# sont bien dans la signature. `--distribution` exige en plus l'autorité
# Developer ID. `notariser` lit APPLE_ID, APPLE_PASSWORD (mot de passe
# d'application) et APPLE_TEAM_ID dans l'environnement, jamais en argument.
set -eu

PHOTOTHEQUE="com.apple.security.personal-information.photos-library"

echouer() { echo "signature-macos : $*" >&2; exit 1; }

verifier() {
  app="$1"; distribution="${2:-}"
  [ -d "$app" ] || echouer "pas d'app à $app"
  # Un drapeau mal orthographié sauterait le contrôle Developer ID en silence.
  case "$distribution" in ""|--distribution) ;; *) echouer "drapeau inconnu : $distribution" ;; esac

  codesign --verify --deep --strict --verbose=2 "$app" \
    || echouer "signature invalide : $app"

  details="$(codesign -d --verbose=2 "$app" 2>&1)"
  echo "$details" | grep -q 'flags=.*runtime' \
    || echouer "runtime durci absent, la notarisation sera refusée"

  # Sans cette clé, le runtime durci rend la photothèque vide sans lever.
  # La clé doit valoir vrai : sa seule présence passerait un <false/>.
  codesign -d --entitlements - --xml "$app" 2>/dev/null | python3 -c '
import plistlib, sys
donnees = sys.stdin.buffer.read()
sys.exit(0 if donnees and plistlib.loads(donnees).get(sys.argv[1]) is True else 1)
' "$PHOTOTHEQUE" \
    || echouer "la signature ne porte pas $PHOTOTHEQUE à vrai : la release se verrait refuser la photothèque"

  if [ "$distribution" = "--distribution" ]; then
    echo "$details" | grep -q '^Authority=Developer ID Application' \
      || echouer "l'app n'est pas signée Developer ID : $(echo "$details" | grep -m1 '^Authority=' || echo 'aucune autorité')"
  fi
  echo "signature : $app"
  echo "$details" | grep -E '^(Authority|TeamIdentifier|Timestamp)=' | head -3
}

notariser() {
  dmg="$1"
  [ -f "$dmg" ] || echouer "pas de DMG à $dmg"
  : "${APPLE_ID:?APPLE_ID manque}" "${APPLE_PASSWORD:?APPLE_PASSWORD manque}" "${APPLE_TEAM_ID:?APPLE_TEAM_ID manque}"

  # `--wait` rend 0 même quand Apple refuse : c'est le statut qui fait foi.
  reponse="$(xcrun notarytool submit "$dmg" --wait --output-format json \
    --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID")"
  statut="$(printf '%s' "$reponse" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("status",""))')"
  ident="$(printf '%s' "$reponse" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id",""))')"
  if [ "$statut" != "Accepted" ]; then
    # Le journal d'Apple nomme le fichier fautif ; sans lui on devine.
    [ -n "$ident" ] && xcrun notarytool log "$ident" \
      --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" >&2 || true
    echouer "notarisation refusée ($statut) : $dmg"
  fi
  xcrun stapler staple "$dmg"
  echo "notarisé et agrafé : $dmg ($ident)"
}

gatekeeper() {
  app="$1"; dmg="$2"
  # Agrafé veut dire vérifiable hors ligne, au premier lancement d'un Mac
  # sans réseau. L'app l'est par Tauri, le DMG par `notariser`.
  xcrun stapler validate "$app" || echouer "aucun ticket agrafé sur l'app"
  xcrun stapler validate "$dmg" || echouer "aucun ticket agrafé sur le DMG"
  spctl --assess --type execute --verbose=2 "$app" 2>&1 | tee /dev/stderr \
    | grep -q 'source=Notarized Developer ID' || echouer "Gatekeeper refuse l'app"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg" 2>&1 \
    | tee /dev/stderr | grep -q 'source=Notarized Developer ID' || echouer "Gatekeeper refuse le DMG"
  echo "Gatekeeper accepte l'app et le DMG"
}

[ "$(uname -s)" = "Darwin" ] || echouer "macOS seulement"
commande="${1:-}"; shift || true
case "$commande" in
  verifier)   [ $# -ge 1 ] || echouer "usage : verifier <app> [--distribution]"; verifier "$@" ;;
  notariser)  [ $# -eq 1 ] || echouer "usage : notariser <dmg>"; notariser "$1" ;;
  gatekeeper) [ $# -eq 2 ] || echouer "usage : gatekeeper <app> <dmg>"; gatekeeper "$1" "$2" ;;
  *) echouer "usage : $0 verifier|notariser|gatekeeper …" ;;
esac
