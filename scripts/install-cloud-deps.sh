#!/usr/bin/env bash
# Les dépendances système que Tauri exige sous Linux, posées au démarrage d'une
# session cloud. Ailleurs ce script sort immédiatement : sur le Mac il n'a rien
# à faire, et sur un runner GitHub c'est check.yml qui les installe, avec la
# même liste de paquets.
#
# Pourquoi un hook plutôt que le « Setup script » de l'environnement cloud : ce
# dernier vit dans une page de réglages sur claude.ai, donc hors du dépôt,
# invisible à la relecture et impossible à versionner. Ici il se lit dans le
# diff. Le prix est qu'il n'est pas mis en cache et coûte donc quelques minutes
# par session neuve. Le jour où ça pèse, recopier la liste de paquets dans le
# champ « Setup script » de l'environnement : ce fichier n'aura plus qu'à
# constater qu'ils sont déjà là, et sortira en une milliseconde.
#
# Il ne sort jamais en erreur. Un hook qui échoue empêche la session de
# démarrer, et une session sans webkit reste utile pour tout ce qui ne compile
# pas colophon-app. Si les paquets manquent vraiment, c'est cargo qui le dira,
# à un endroit où on peut le lire.

# Hors session cloud : rien à faire.
[ -n "${CLAUDE_CODE_REMOTE_SESSION_ID:-}" ] || exit 0
command -v apt-get >/dev/null 2>&1 || exit 0

# Déjà présents (session reprise, environnement mis en cache) : ne rien refaire.
if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  echo "colophon : dépendances Tauri déjà en place"
  exit 0
fi

# La VM tourne en root la plupart du temps ; sinon on emprunte l'élévation
# habituelle, et si elle n'existe pas on tente quand même, apt dira non.
PRIV=""
if [ "$(id -u)" -ne 0 ]; then
  command -v sudo >/dev/null 2>&1 && PRIV="sudo"
fi

echo "colophon : installation des dépendances Tauri (Linux)"
$PRIV apt-get update -qq 2>&1 | tail -1
$PRIV apt-get install -y -qq \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  libgtk-3-dev 2>&1 | tail -3

if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  echo "colophon : dépendances Tauri posées"
else
  echo "colophon : dépendances Tauri ABSENTES, cargo build échouera sur colophon-app" >&2
fi
exit 0
