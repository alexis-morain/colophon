#!/usr/bin/env bash
# Régénère les fiches des trois jeux de référence : ce que le scan et
# l'analyse mesurent, versionné, pour que le linter d'albums tourne là où les
# 5,7 Go de photos ne sont pas.
#
# À relancer quand l'analyse change — hashes, netteté, exposition, visages,
# dimensions d'origine. Rien n'oblige à y penser : l'assertion d'identité de
# check.sh rougit quand les fiches ont vieilli, c'est son second rôle.
set -euo pipefail
cd "$(dirname "$0")/.."

TESTSETS="${COLOPHON_TESTSETS:-$HOME/Pictures/colophon-testsets}"
FICHES="crates/colophon-core/fiches"
TRAVAIL=".albums/fiches"
JEUX="corse-2013 mauritanie-2019 random-2024"

if [ ! -d "$TESTSETS" ]; then
  echo "jeux de référence absents ($TESTSETS) : les fiches se relèvent sur les" >&2
  echo "photos, et elles seules. C'est la seule opération de ce dépôt qui les exige." >&2
  exit 1
fi

cargo build --release
mkdir -p "$FICHES"

total=0
for jeu in $JEUX; do
  if [ ! -d "$TESTSETS/$jeu" ]; then
    echo "jeu $jeu absent de $TESTSETS : les trois se relèvent ensemble" >&2
    exit 1
  fi
  ./target/release/colophon "$TESTSETS/$jeu" -o "$TRAVAIL/$jeu" \
    --dump-fiches "$FICHES/$jeu.json"
  total=$(( total + $(wc -c < "$FICHES/$jeu.json") ))
done

echo "fiches : $(( total / 1024 )) Ko pour les trois jeux, dans $FICHES"
# Le méga-octet n'est pas une limite, c'est une alarme. Au-delà, un fichier
# versionné cesse de se lire à l'œil et perd la moitié de son intérêt : c'est
# un arbitrage à prendre, jamais une compression à passer en douce.
if [ "$total" -gt 1048576 ]; then
  echo "  au-dessus du méga-octet : à arbitrer, pas à compresser en silence" >&2
fi
