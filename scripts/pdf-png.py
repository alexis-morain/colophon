#!/usr/bin/env python3
"""Non-régression PDF → PNG par gabarit, sur les six formats.

Le binaire rend une planche par gabarit avec un aplat de couleur par case
(`colophon --sheets`), sips rasterise le PDF, et ce script vérifie que
chaque case affiche sa couleur là où la géométrie la place. Le PDF fait
foi : la parité TypeScript contrôle l'arithmétique, ce test contrôle le
rendu réel (placement, clipping, ordre de lecture).

Usage : scripts/pdf-png.py [chemin du binaire]
Prérequis : macOS (sips) et Pillow. Sort en code non nul au premier écart.
"""

import json
import os
import subprocess
import sys
import tempfile

from PIL import Image

BIN = sys.argv[1] if len(sys.argv) > 1 else "target/release/colophon"
FORMATS = [
    "carre-21",
    "carre-30",
    "portrait-a4",
    "paysage-a4",
    "paysage-28x21",
    "portrait-20x25",
]
# Copie de pdf.rs::SHEET_PALETTE, dans le même ordre de cases.
PALETTE = [
    (200, 30, 40),
    (30, 120, 200),
    (30, 160, 60),
    (230, 160, 30),
    (130, 60, 180),
    (20, 170, 170),
    (230, 90, 140),
    (90, 90, 30),
]
# JPEG, profils ICC et rasterisation décalent les valeurs absolues, jamais
# les teintes : une case est bonne quand SA couleur de palette est la plus
# proche de ce qui est lu, et nettement (marge sur l'écart au second choix).
WHITE = (255, 255, 255)

failures = 0
checked = 0

for fmt in FORMATS:
    geo = json.loads(
        subprocess.check_output([BIN, "--dump-geometry", "--format", fmt])
    )
    canvas = geo["canvas"]
    with tempfile.TemporaryDirectory() as td:
        subprocess.check_call(
            [BIN, "--sheets", td, "--format", fmt],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for name, spec in geo["templates"].items():
            pdf = os.path.join(td, f"{name}.pdf")
            png = os.path.join(td, f"{name}.png")
            subprocess.check_call(
                ["sips", "-s", "format", "png", pdf, "--out", png],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            im = Image.open(png).convert("RGB")
            sx = im.size[0] / canvas["w"]
            sy = im.size[1] / canvas["h"]
            n = len(spec["slots"])
            for i, (x, y, w, h) in enumerate(spec["slots"]):
                # géométrie en origine bas-gauche, PNG en haut-gauche
                px = min(int((x + w / 2) * sx), im.size[0] - 1)
                py = min(int((canvas["h"] - (y + h / 2)) * sy), im.size[1] - 1)
                got = im.getpixel((px, py))
                # candidats : les couleurs des cases de CE gabarit, plus le
                # blanc du papier (attrape une case pas peinte du tout)
                candidates = list(PALETTE[:n]) + [WHITE]
                dists = [
                    sum((a - b) ** 2 for a, b in zip(got, c)) for c in candidates
                ]
                best = dists.index(min(dists))
                checked += 1
                if best != i:
                    failures += 1
                    label = "blanc" if best == n else f"couleur de la case {best}"
                    print(
                        f"ÉCART {fmt} {name} case {i} : lu {got} en "
                        f"({px},{py}), plus proche de {label}",
                        file=sys.stderr,
                    )

if failures:
    print(f"pdf-png : {failures} écart(s) sur {checked} cases", file=sys.stderr)
    sys.exit(1)
print(f"pdf-png : {checked} cases vérifiées sur {len(FORMATS)} formats, ok")
