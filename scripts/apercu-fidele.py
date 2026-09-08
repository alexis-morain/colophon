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

L'aperçu est toujours en planches doubles : c'est ce que l'écran montre. Le
tirage doit donc l'être aussi pour que la comparaison ait un sens, et il faut
le rendre sous un profil qui relie des planches — `--profil lulu`, le seul qui
en rende un intérieur sans couverture dedans. Sous `cloudprinter` ou `prodigi`
le tirage sort page par page (`core::imposition`) et les deux fichiers n'ont ni
le même nombre de pages ni le même format ; le script le dit plutôt que de
comparer des grilles qui ne se regardent pas.

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
    # Deux fichiers du même livre peuvent avoir des pages qui n'ont rien à voir :
    # sous un profil qui relie page par page, le tirage sort en pages simples et
    # l'aperçu reste en planches doubles. Le compte, lui, se réconcilierait tout
    # seul (48 planches contre 96 pages ressemble à deux couvertures de chaque
    # côté d'un bloc de 48), donc c'est la largeur qui tranche, pas le nombre.
    la, lb = a[0].get_width(), b[0].get_width()
    if abs(la - lb) > 1.0:
        print(
            f"formats inconciliables : aperçu {la:.0f} pt, impression {lb:.0f} pt. "
            "Rendez le tirage sous un profil qui relie des planches (--profil lulu).",
            file=sys.stderr,
        )
        return 1
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
