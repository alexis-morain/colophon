#!/usr/bin/env python3
"""L'aperçu fidèle tient-il sa promesse : même géométrie que le PDF imprimé.

L'écran lit `album.pdf`, l'imprimeur reçoit `album-print.pdf`. Les deux
sortent du même `album.json` et du même moteur, mais pas de la même source
d'images (vignettes contre originaux à 300 dpi) : le test compare donc la
géométrie, pas les pixels. Chaque page des deux fichiers est rasterisée,
réduite à une grille de luminance, et les deux grilles doivent coïncider.

Ce que ça attrape : une case décalée, un rognage différent, une légende
placée ailleurs, une page de colophon absente d'un côté, un fond perdu
appliqué à l'un et pas à l'autre.

Usage : scripts/apercu-fidele.py <dossier d'album> [écart maximal]
Prérequis : pypdfium2 et Pillow (venv de session), et les deux PDF rendus.
"""

import sys
from pathlib import Path

import pypdfium2 as pdfium
from PIL import Image

# Grille de comparaison. 96 colonnes sur une planche double : une case mal
# placée d'un millimètre bouge d'au moins une colonne.
GRILLE = (96, 48)
# Écart moyen toléré, sur 255. Les deux fichiers n'ont ni la même définition
# ni le même JPEG : un fond identique ne donne jamais zéro.
DEFAUT_SEUIL = 14.0


def grille(page) -> Image.Image:
    img = page.render(scale=0.5).to_pil().convert("L")
    return img.resize(GRILLE, Image.BILINEAR)


def ecart(a: Image.Image, b: Image.Image) -> float:
    pa, pb = a.load(), b.load()
    total = 0
    for y in range(GRILLE[1]):
        for x in range(GRILLE[0]):
            total += abs(pa[x, y] - pb[x, y])
    return total / (GRILLE[0] * GRILLE[1])


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    dossier = Path(sys.argv[1])
    seuil = float(sys.argv[2]) if len(sys.argv) > 2 else DEFAUT_SEUIL

    apercu = dossier / "album.pdf"
    imprime = dossier / "album-print.pdf"
    for f in (apercu, imprime):
        if not f.is_file():
            print(f"{f} absent : rendez l'aperçu et --print d'abord", file=sys.stderr)
            return 2

    a = pdfium.PdfDocument(apercu)
    b = pdfium.PdfDocument(imprime)
    # Un imprimeur qui relie un seul fichier reçoit la couverture en première
    # et dernière page : on aligne sur la fin, le bloc intérieur étant commun.
    decalage = (len(b) - len(a)) // 2
    if len(a) + 2 * decalage != len(b):
        print(
            f"pages inconciliables : aperçu {len(a)}, impression {len(b)}",
            file=sys.stderr,
        )
        return 1

    pire = 0.0
    pire_page = 0
    for i in range(len(a)):
        d = ecart(grille(a[i]), grille(b[i + decalage]))
        if d > pire:
            pire, pire_page = d, i + 1

    etat = "ok" if pire <= seuil else "ÉCART"
    print(
        f"aperçu fidèle {dossier.name} : {len(a)} planches, "
        f"écart maximal {pire:.1f}/255 planche {pire_page} ({etat})"
    )
    return 0 if pire <= seuil else 1


if __name__ == "__main__":
    sys.exit(main())
