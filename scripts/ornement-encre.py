#!/usr/bin/env python3
"""L'encre d'un ornement tombe dans sa boîte, mesurée sur le papier.

Le pli, la coupe, le prévol et le linter ne mesurent **jamais** que le
rectangle d'un objet libre. Pour un bloc de texte c'est une approximation
assumée — une ligne peut déborder, et la scène le dit. Pour un ornement c'est
un invariant : sa boîte garde le rapport de son `viewBox`, donc la boîte
**est** l'encre. Si elle cessait de l'être, le prévol laisserait partir un
fleuron que la guillotine traverse en jurant qu'il est au large.

Le raster est le seul juge possible : il lit ce qui est imprimé, pas ce que le
code croit avoir écrit. On pose donc un ornement seul sur une planche, on
exporte, on rasterise, et on compare la boîte des pixels sombres à la boîte
d'`album.json`.

Usage : scripts/ornement-encre.py [chemin du binaire]
Prérequis : macOS (sips) et Pillow, comme `pdf-png.py`. Sort non nul au
premier écart.
"""

import json
import os
import subprocess
import sys
import tempfile

from PIL import Image

BIN = sys.argv[1] if len(sys.argv) > 1 else "target/release/colophon"
FORMAT = "carre-21"
# Voir plus bas : il faut un intérieur de planches doubles, et lui seul en rend.
PROFIL = "lulu"
# Un demi-millimètre : la tolérance que le prompt de 6.3 s1 fixe, et elle est
# large devant le pas du raster (0,21 mm à 300 ppi) comme devant
# l'anticrénelage d'un bord courbe.
TOLERANCE_MM = 0.5
# Dix pixels par millimètre. sips rasterise un PDF à 72 ppp par défaut, soit
# 0,35 mm par pixel : une tolérance d'un demi-millimètre y vaudrait un pixel
# et demi, et l'anticrénelage d'un bord courbe la ferait battre au hasard.
# `-Z` demande la rasterisation à la taille voulue, pas un rééchantillonnage.
PX_PAR_MM = 10

# Trois poses, et elles ne prouvent pas la même chose. Droite : l'échelle et le
# renversement du `y` du SVG. Tournée : que la rotation garde l'encre dans le
# rectangle que `corners` mesure — c'est celle-là que le prévol lit.
#
# **La flèche est la sonde**, et sa présence n'est pas un caprice : elle touche
# ses quatre bords, là où un filet en losange ou à renflement se termine en
# pointe et absorbe silencieusement un millimètre d'erreur. Mesuré le 06/09 en
# déplaçant la matrice de trois points dans l'émetteur : le losange passait de
# +0,00 à +0,20 mm et laissait le test vert, la flèche à +1,00 et le rougissait.
POSES = [
    {"id": "filet-losange", "x": 40.0, "y": 60.0, "w": 100.0, "h": 5.0, "angle": 0.0},
    {"id": "filet-flare", "x": 220.0, "y": 90.0, "w": 70.0, "h": 4.5, "angle": -46.0},
    {"id": "filet-fleche", "x": 180.0, "y": 140.0, "w": 12.0, "h": 17.0, "angle": 0.0},
]


def coins(o):
    """Les quatre coins d'un objet tourné, repère du moteur. Écrit à la main,
    exprès : réutiliser `scene::corners` ferait mesurer le code par lui-même."""
    import math

    cx, cy = o["x"] + o["w"] / 2, o["y"] + o["h"] / 2
    a = math.radians(o["angle"])
    pts = [
        (o["x"], o["y"]),
        (o["x"] + o["w"], o["y"]),
        (o["x"] + o["w"], o["y"] + o["h"]),
        (o["x"], o["y"] + o["h"]),
    ]
    return [
        (
            cx + (x - cx) * math.cos(a) - (y - cy) * math.sin(a),
            cy + (x - cx) * math.sin(a) + (y - cy) * math.cos(a),
        )
        for x, y in pts
    ]


geo = json.loads(subprocess.check_output([BIN, "--dump-geometry", "--format", FORMAT]))
media = geo["media"]
trim = geo["trim_mm"]
bleed = geo["bleed_mm"]

album = {
    # `version` est le champ de schéma, et 3 est celui qui porte les objets
    # libres. Un album refusé ici est un album que le moteur refuserait à
    # quelqu'un : l'erreur remonte telle quelle, plus bas, plutôt que d'être
    # avalée.
    "version": 3,
    "title": "ornement",
    "root": ".",
    "trim_mm": {"w": trim["w"], "h": trim["h"]},
    "bleed_mm": bleed,
    "spreads": [
        {
            "template": "vide",
            "slots": [],
            "objets": [
                {
                    "x": p["x"],
                    "y": p["y"],
                    "w": p["w"],
                    "h": p["h"],
                    **({"angle": p["angle"]} if p["angle"] else {}),
                    "type": "ornement",
                    "pack": "colophon",
                    "id": p["id"],
                }
                for p in POSES
            ],
        }
    ],
}

echecs = 0
with tempfile.TemporaryDirectory() as td:
    dossier = os.path.join(td, "album")
    os.makedirs(dossier)
    with open(os.path.join(dossier, "album.json"), "w", encoding="utf-8") as f:
        json.dump(album, f)
    # Le profil est nommé, et il l'est pour une raison : ce qu'on mesure ici
    # est l'encre d'un ornement dans sa boîte **sur une planche**, dans le
    # repère que `--dump-geometry` vient de rendre. Un imprimeur qui relie page
    # par page reçoit la même planche coupée en deux (`core::imposition`), et
    # la première page du fichier n'est alors plus la planche entière. `lulu`
    # est le seul profil qui rende un intérieur de planches doubles sans y
    # glisser de couverture ; le défaut, `cloudprinter`, découpe.
    rendu = subprocess.run(
        [BIN, "--print", "--profil", PROFIL, "-o", dossier], capture_output=True, text=True
    )
    if rendu.returncode != 0:
        print(f"ornement-encre : le tirage a refusé\n{rendu.stderr}", file=sys.stderr)
        sys.exit(1)
    pdfs = sorted(n for n in os.listdir(dossier) if n.endswith(".pdf"))
    tirage = [n for n in pdfs if "print" in n] or pdfs
    if not tirage:
        print("ornement-encre : aucun PDF produit", file=sys.stderr)
        sys.exit(1)
    pdf = os.path.join(dossier, tirage[0])

    png = os.path.join(td, "planche.png")
    subprocess.check_call(
        ["sips", "-s", "format", "png", "-Z", str(int(media["w"] * PX_PAR_MM)),
         pdf, "--out", png],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    # Le papier est blanc. Un PNG à canal alpha converti tel quel rend du noir
    # sous la transparence, et toute la planche se lit alors comme de l'encre :
    # le test passerait en mesurant la page entière.
    brut = Image.open(png)
    fond = Image.new("RGB", brut.size, (255, 255, 255))
    fond.paste(brut, mask=brut.split()[-1] if brut.mode == "RGBA" else None)
    im = fond.convert("L")
    px_par_mm = im.width / media["w"]

    for p in POSES:
        cs = coins(p)
        xs, ys = [c[0] for c in cs], [c[1] for c in cs]
        # Le repère du raster descend, celui du moteur monte.
        gx0, gx1 = min(xs), max(xs)
        gy0, gy1 = media["h"] - max(ys), media["h"] - min(ys)
        marge = TOLERANCE_MM * px_par_mm
        boite = (
            max(0, int((gx0 - 2) * px_par_mm)),
            max(0, int((gy0 - 2) * px_par_mm)),
            min(im.width, int((gx1 + 2) * px_par_mm)),
            min(im.height, int((gy1 + 2) * px_par_mm)),
        )
        vignette = im.crop(boite)
        sombres = [
            (x + boite[0], y + boite[1])
            for y in range(vignette.height)
            for x in range(vignette.width)
            if vignette.getpixel((x, y)) < 128
        ]
        if not sombres:
            print(f"ornement-encre : {p['id']} n'a rien imprimé", file=sys.stderr)
            echecs += 1
            continue
        ex0 = min(s[0] for s in sombres)
        ex1 = max(s[0] for s in sombres)
        ey0 = min(s[1] for s in sombres)
        ey1 = max(s[1] for s in sombres)
        ecarts = {
            "gauche": gx0 * px_par_mm - ex0,
            "droite": ex1 - gx1 * px_par_mm,
            "haut": gy0 * px_par_mm - ey0,
            "bas": ey1 - gy1 * px_par_mm,
        }
        pires = {k: v / px_par_mm for k, v in ecarts.items() if v > marge}
        if pires:
            print(
                f"ornement-encre : {p['id']} déborde de "
                + ", ".join(f"{k} {v:.2f} mm" for k, v in pires.items()),
                file=sys.stderr,
            )
            echecs += 1
        else:
            # Négatif = l'encre s'arrête en deçà du bord, ce qui est normal :
            # un filet en losange se termine en pointe, et sa pointe est plus
            # fine qu'un pixel avant de l'être tout à fait.
            debord = max(ecarts.values()) / px_par_mm
            print(
                f"  ok   {p['id']} à {p['angle']:.0f}° : encre dans sa boîte, "
                f"pire bord à {debord:+.2f} mm"
            )

if echecs:
    sys.exit(1)
print(f"ornement-encre : {len(POSES)} poses, encre dans la boîte")
