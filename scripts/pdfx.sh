#!/usr/bin/env bash
# Conformance of the exported PDFs, measured rather than asserted.
#
# veraPDF ships PDF/A and PDF/UA profiles and no PDF/X one — no free
# validator does. So what gets measured here is PDF/A-2b, whose structural
# core is the one PDF/X-4 asks for too: embedded fonts, an OutputIntent
# carrying a real ICC profile, a well-formed XMP packet that agrees with
# /Info, no encryption, nothing borrowed from the reader. The rules that
# belong to X-4 alone are checked by the Rust tests in pdfx.rs, which reopen
# the file the writer just wrote.
#
#   ./scripts/pdfx.sh              the template sheets, every format
#   ./scripts/pdfx.sh full         the above plus the three test sets, print
#                                  render included (slow, several minutes)
#
# Exits non-zero on the first file veraPDF refuses.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v verapdf >/dev/null; then
  echo "pdfx : veraPDF absent (brew install verapdf), mesure impossible" >&2
  exit 2
fi

FORMATS="carre-21 carre-30 portrait-a4 paysage-a4 paysage-28x21 portrait-20x25"
OUT=".albums/pdfx"
FULL="${1:-}"
fail=0

check() { # file, label
  if verapdf -f 2b --format text "$1" 2>/dev/null | grep -q '^PASS'; then
    echo "  ok   $2"
  else
    echo "  FAIL $2" >&2
    verapdf -f 2b --format text -v "$1" 2>/dev/null | sed 's/^/       /' >&2
    fail=1
  fi
}

echo "gabarits, six formats :"
for f in $FORMATS; do
  rm -rf "$OUT/$f"
  ./target/release/colophon --sheets "$OUT/$f" --format "$f" >/dev/null 2>&1
  n=0
  bad=0
  for pdf in "$OUT/$f"/*.pdf; do
    n=$((n + 1))
    verapdf -f 2b --format text "$pdf" 2>/dev/null | grep -q '^PASS' || {
      bad=$((bad + 1))
      echo "  FAIL $f/$(basename "$pdf")" >&2
      verapdf -f 2b --format text -v "$pdf" 2>/dev/null | sed 's/^/       /' >&2
    }
  done
  [ "$bad" -eq 0 ] && echo "  ok   $f : $n gabarits" || fail=1
done

# The real thing: composed albums, real photos, captions in Source Sans 3,
# and the 300 dpi render that is what actually reaches a printer.
TESTSETS="$HOME/Pictures/colophon-testsets"
if [ "$FULL" = "full" ] && [ -d "$TESTSETS" ]; then
  echo "albums composés :"
  for set in corse-2013 mauritanie-2019 random-2024; do
    [ -d "$TESTSETS/$set" ] || continue
    for f in $FORMATS; do
      dir="$OUT/albums/$set-$f"
      rm -rf "$dir"
      ./target/release/colophon "$TESTSETS/$set" -o "$dir" --format "$f" \
        >/dev/null 2>&1
      check "$dir/album.pdf" "$set $f aperçu"
      ./target/release/colophon --print -o "$dir" >/dev/null 2>&1
      check "$dir/album-print.pdf" "$set $f impression"
      # The cover travels as its own file at the suppliers that ask for two.
      ./target/release/colophon --cover --profil cloudprinter -o "$dir" \
        >/dev/null 2>&1
      check "$dir/album-cover.pdf" "$set $f couverture"
    done
  done
fi

if [ "$fail" -ne 0 ]; then
  echo "pdfx : au moins un PDF refusé par veraPDF" >&2
  exit 1
fi
echo "pdfx : tout passe en PDF/A-2b"
