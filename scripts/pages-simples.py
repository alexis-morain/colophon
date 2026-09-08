#!/usr/bin/env python3
"""La découpe en pages simples ne déplace rien, et le pli saigne.

Le même album sort dans les deux formes : en planches doubles chez un
imprimeur qui les impose lui-même, en pages simples chez celui qui relie une
page de PDF par page de livre. La question que ce script pose au papier est la
seule qui compte : **est-ce le même livre**.

Le raster est le seul juge possible. Les tests Rust mesurent des rectangles, et
un rectangle juste posé sur la mauvaise page reste un rectangle juste ; une
page rasterisée à côté de la moitié de planche dont elle sort, elle, ne ment
pas. Quatre choses sont mesurées :

1. **la géométrie** : n planches de `2·trim + 2·fond perdu` × `trim + 2·fond
   perdu` d'un côté, 2n pages de `trim + fond perdu + pli` de l'autre ;
2. **l'imposition** : page par page, la page simple et la moitié de planche
   dont elle sort doivent coïncider — p1 est le recto de la planche 1, p2k la
   gauche de la planche k+1, p2k+1 sa droite, p2n une blanche. Les cases qui
   atteignent le pli sont masquées : elles changent, exprès, et c'est le
   point 3 ;
3. **le fond perdu du pli** : une photo qui atteignait le pli déborde
   maintenant de trois millimètres au-delà, et cette bande doit prolonger la
   photo au lieu de rester blanche ;
4. **les deux pages que l'imposition fabrique** : la moitié gauche de la
   première planche, qui ne s'imprime pas, doit être blanche, et la dernière
   page du fichier aussi.

**Deux précautions, et elles ont coûté cher à trouver** (mesurées le 08/09).
D'abord l'échelle du raster se déduit de la taille voulue, jamais l'inverse :
pdfium arrondit la taille du bitmap vers le haut puis y étire la page, si bien
qu'une planche demandée à 3408 pixels sort à 3409 et se retrouve dessinée un
demi-pixel trop large au bout. Ensuite, les coordonnées d'un PDF Colophon sont
écrites en points à deux décimales : la même arête vaut 613,70 dans la planche
et 18,43 dans la page, deux nombres qui disent le même millimètre **à deux
microns près**. Deux microns ne se voient pas sur du papier, mais ils suffisent
à changer la phase du rééchantillonnage d'une photo à 300 ppi, et à faire
glisser tout un raster d'une colonne. On recale donc d'un pixel au plus, puis
on compare des moyennes au millimètre carré : un vrai défaut d'imposition vaut
au moins trois millimètres, et aucune de ces deux précautions ne peut en
cacher un.

**La sonde doit mordre**, sinon elle ne mesure rien : `--mutant` décale
l'imposition d'une page, et le script doit alors échouer bruyamment.
`--avec-mutant` fait les deux passes sur **un seul rendu** — la mesure, puis le
mensonge — et c'est cette forme-là que le gate appelle : deux invocations
paieraient deux fois le tirage 300 dpi pour ne rien apprendre de plus.

Usage :
    scripts/pages-simples.py <dossier d'album> [--mutant|--avec-mutant] [--px-par-mm N]

Prérequis : pypdfium2 et Pillow (venv de session), et le binaire release. Le
script rend les deux fichiers lui-même, dans un dossier temporaire : il ne
laisse rien derrière lui et ne dépend d'aucun export antérieur.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pypdfium2 as pdfium
from PIL import Image, ImageChops, ImageStat

Image.MAX_IMAGE_PIXELS = None

BIN = "target/release/colophon"
# Le profil qui relie des planches doubles sans glisser de couverture dans
# l'intérieur, et celui qui relie page par page. Nommés, pas devinés : le
# défaut de la ligne de commande découpe désormais.
PROFIL_PLANCHES = "lulu"
PROFIL_SIMPLES = "cloudprinter"

# Huit pixels par millimètre : les deux fichiers tombent sur des tailles
# entières (426 × 8 = 3408, 216 × 8 = 1728) et un millimètre reste un carré de
# 64 pixels à moyenner.
PX_PAR_MM = 8
# Le recalage cherche dans un pixel, pas plus. Voir l'en-tête : deux microns
# d'écart de coordonnée suffisent à faire glisser un raster d'une colonne.
ALIGNEMENT_PX = 1
# Ce qui reste après recalage, une fois moyenné au millimètre carré. Mesuré le
# 08/09 sur corse-2013 : pire moyenne 0,64/255 et pire millimètre 22/255, quand
# le mutant sort à 166 et 250. Ces seuils ne sont pas un réglage fin — ils sont
# à quatre fois le bruit mesuré et à cinquante fois sous le mensonge.
SEUIL_MOYEN = 3.0
SEUIL_PIRE_MM = 60.0
# Le fond perdu du pli prolonge la photo : la couleur moyenne de la bande neuve
# doit rester proche des trois derniers millimètres imprimés à l'intérieur de
# la coupe. Une bande restée blanche sortirait très au-delà.
SEUIL_PLI = 40.0


def rendre(album: Path, profil: str, sortie: Path) -> Path:
    """Un tirage 300 dpi sous un profil nommé."""
    subprocess.check_call(
        [BIN, "--print", "--profil", profil, "-o", str(album)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    (album / "album-print.pdf").rename(sortie)
    return sortie


class Fichier:
    """Un PDF dont on tire une page à la fois, à la taille voulue.

    À la demande, et pas d'avance : un album de cinquante planches rasterisé
    d'un coup pèse près de deux gigaoctets, pour ne regarder que deux pages à
    la fois.
    """

    def __init__(self, chemin: Path, taille):
        self.doc = pdfium.PdfDocument(chemin)
        self.nom = chemin.name
        self.taille = taille

    def __len__(self):
        return len(self.doc)

    def page(self, i: int) -> Image.Image:
        pg = self.doc[i]
        im = pg.render(scale=self.taille[0] / pg.get_width()).to_pil().convert("RGB")
        if abs(im.width - self.taille[0]) > 1 or abs(im.height - self.taille[1]) > 1:
            raise SystemExit(f"{self.nom} page {i + 1} : {im.size} au lieu de {self.taille}")
        return im.crop((0, 0, self.taille[0], self.taille[1]))


def mm(px_par_mm: int, v: float) -> int:
    return int(round(v * px_par_mm))


def au_millimetre(im: Image.Image, px_par_mm: int) -> Image.Image:
    """La même image, un pixel par millimètre, par moyenne de boîte."""
    return im.resize((im.width // px_par_mm, im.height // px_par_mm), Image.BOX)


def compare(a: Image.Image, b: Image.Image, px_par_mm: int):
    """Écart moyen et pire millimètre, après le meilleur recalage d'un pixel.

    Renvoie `(moyenne sur 255, pire millimètre sur 255, décalage)`.
    """
    if a.size != b.size:
        return (255.0, 255.0, None)
    f = ALIGNEMENT_PX
    w, h = a.size
    coeur = au_millimetre(a.crop((f, 0, w - f, h)), px_par_mm)
    meilleur = (255.0, 255.0, None)
    for dx in range(-f, f + 1):
        d = ImageChops.difference(
            coeur, au_millimetre(b.crop((f + dx, 0, w - f + dx, h)), px_par_mm)
        )
        moyen = sum(ImageStat.Stat(d).mean) / 3.0
        if moyen < meilleur[0]:
            meilleur = (moyen, float(max(d.getextrema()[c][1] for c in range(3))), dx)
    return meilleur


def blanche(im: Image.Image) -> bool:
    return all(lo >= 250 for lo, _ in im.getextrema())


def cases_au_pli(scene, pli: float, cote: str):
    """Les cases de cette moitié dont le bord touche le pli. Ce sont les seules
    que l'export recadre, donc les seules que la comparaison masque."""
    for o in scene["objects"]:
        if o["role"]["role"] != "photo":
            continue
        r = o["rect"]
        bord = r["x"] + r["w"] if cote == "gauche" else r["x"]
        if abs(bord - pli) < 1e-9:
            yield r


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    mutant = "--mutant" in sys.argv
    avec_mutant = "--avec-mutant" in sys.argv
    px_par_mm = PX_PAR_MM
    for i, a in enumerate(sys.argv):
        if a == "--px-par-mm":
            px_par_mm = int(sys.argv[i + 1])
    if not args:
        print(__doc__, file=sys.stderr)
        return 2
    album = Path(args[0])

    modele = json.loads((album / "album.json").read_text())
    trim, bleed = modele["trim_mm"], modele["bleed_mm"]
    planches = len(modele["spreads"])
    media_w = trim["w"] * 2 + bleed * 2
    media_h = trim["h"] + bleed * 2
    # Le pli vient du profil, jamais d'ici : le lire garde ce script juste chez
    # un imprimeur qui en demanderait un autre.
    profil = next(
        p
        for p in json.loads(subprocess.check_output([BIN, "--profils-json"], text=True))
        if p["id"] == PROFIL_SIMPLES
    )
    pli_mm = profil["bleed_mm"]["dos"]
    page_w = trim["w"] + bleed + pli_mm
    pli = media_w / 2.0

    # Ce que chaque planche porte, dit par la composition et non par l'export :
    # si l'imposition se trompe, ce dump reste juste, ce qui est exactement ce
    # qu'on attend d'une référence.
    scenes = json.loads(
        subprocess.check_output([BIN, "--dump-scene", str(album / "album.json")], text=True)
    )


    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        doubles = Fichier(
            rendre(album, PROFIL_PLANCHES, td / "planches.pdf"),
            (mm(px_par_mm, media_w), mm(px_par_mm, media_h)),
        )
        simples = Fichier(
            rendre(album, PROFIL_SIMPLES, td / "simples.pdf"),
            (mm(px_par_mm, page_w), mm(px_par_mm, media_h)),
        )

        def mesurer(decalage: int) -> int:
            """Toute la mesure, pour un décalage d'imposition donné.

            `decalage` vaut zéro pour la vérité et un pour le mensonge. Les deux
            passes lisent **les mêmes rendus** : un tirage 300 dpi coûte dix
            secondes et deux gigaoctets, et le mutant n'apprend rien de plus en
            le repayant.
            """
            echecs = 0
            p0, s0 = doubles.page(0), simples.page(0)
            for lu, veut, quoi in [
                (len(doubles), planches, "planches doubles"),
                (len(simples), planches * 2, "pages simples"),
                (p0.width, mm(px_par_mm, media_w), "largeur d'une planche"),
                (p0.height, mm(px_par_mm, media_h), "hauteur d'une planche"),
                (s0.width, mm(px_par_mm, page_w), "largeur d'une page"),
                (s0.height, mm(px_par_mm, media_h), "hauteur d'une page"),
            ]:
                etat = "ok  " if lu == veut else "ÉCART"
                print(f"  {etat} {quoi} : {lu} (attendu {veut})")
                echecs += lu != veut
            if echecs:
                return echecs

            # La moitié gauche de la première planche ne s'imprime pas : c'est
            # ce qui met le faux-titre en page un. Elle doit donc être blanche,
            # et le prévol le refuse quand elle ne l'est pas.
            vide = blanche(p0.crop((0, 0, mm(px_par_mm, pli), p0.height)))
            print(
                f"  {'ok  ' if vide else 'ÉCART'} moitié gauche de la planche 1 : "
                f"{'blanche' if vide else 'elle porte de l’encre'}"
            )
            echecs += not vide

            pire_moyen, pire_mm, pire_page, glissees, masquees = 0.0, 0.0, "", 0, 0
            bandes, pire_pli, pire_pli_page = 0, 0.0, ""
            for s in range(planches):
                planche = doubles.page(s)
                for cote in ("gauche", "droite"):
                    if s == 0 and cote == "gauche":
                        continue  # la page qui ne s'imprime pas
                    idx = ((2 * s - 1) if cote == "gauche" else (2 * s)) + decalage
                    if idx >= len(simples):
                        continue
                    page = simples.page(idx)

                    if cote == "gauche":
                        a = planche.crop((0, 0, mm(px_par_mm, pli), planche.height))
                        b = page.crop((0, 0, mm(px_par_mm, trim["w"] + bleed), page.height))
                    else:
                        a = planche.crop((mm(px_par_mm, pli), 0, planche.width, planche.height))
                        b = page.crop((mm(px_par_mm, pli_mm), 0, page.width, page.height))

                    # Le fond perdu du pli, avant de masquer les cases.
                    for r in cases_au_pli(scenes[s], pli, cote):
                        y0 = mm(px_par_mm, media_h - (r["y"] + r["h"]))
                        y1 = y0 + mm(px_par_mm, r["h"])
                        large = mm(px_par_mm, pli_mm)
                        if cote == "gauche":  # la bande neuve est au bord droit
                            neuf = (page.width - large, y0, page.width, y1)
                            dedans = (page.width - 2 * large, y0, page.width - large, y1)
                        else:
                            neuf = (0, y0, large, y1)
                            dedans = (large, y0, 2 * large, y1)
                        ma = ImageStat.Stat(page.crop(neuf)).mean
                        mb = ImageStat.Stat(page.crop(dedans)).mean
                        d = sum(abs(x - y) for x, y in zip(ma, mb)) / 3.0
                        bandes += 1
                        if d > pire_pli:
                            pire_pli, pire_pli_page = d, f"planche {s + 1} {cote}"

                    # L'imposition. Une case qui atteint le pli est recadrée
                    # pour saigner : elle change, et on vient de la mesurer.
                    for r in cases_au_pli(scenes[s], pli, cote):
                        masquees += 1
                        x0 = mm(px_par_mm, r["x"] - (0 if cote == "gauche" else pli))
                        y0 = mm(px_par_mm, media_h - (r["y"] + r["h"]))
                        cache = (x0, y0, x0 + mm(px_par_mm, r["w"]), y0 + mm(px_par_mm, r["h"]))
                        for im in (a, b):
                            im.paste((255, 255, 255), cache)

                    moyen, maxi, dx = compare(a, b, px_par_mm)
                    glissees += bool(dx)
                    if moyen > pire_moyen:
                        pire_moyen, pire_mm, pire_page = moyen, maxi, f"planche {s + 1} {cote}"

            etat = "ok  " if pire_moyen <= SEUIL_MOYEN and pire_mm <= SEUIL_PIRE_MM else "ÉCART"
            print(
                f"  {etat} imposition : {planches * 2 - 1} pages, pire moyenne "
                f"{pire_moyen:.2f}/255 et pire millimètre {pire_mm:.0f}/255"
                f"{' sur ' + pire_page if pire_page else ''} "
                f"({masquees} cases au pli masquées, {glissees} pages recalées d'un pixel)"
            )
            echecs += etat != "ok  "

            etat = "ok  " if bandes and pire_pli <= SEUIL_PLI else "ÉCART"
            print(
                f"  {etat} fond perdu du pli : {bandes} bandes, pire écart à la photo "
                f"{pire_pli:.0f}/255{' sur ' + pire_pli_page if pire_pli_page else ''}"
            )
            echecs += etat != "ok  " or bandes == 0

            # La blanche de queue, qui ferme le bloc.
            vide = blanche(simples.page(len(simples) - 1))
            print(
                f"  {'ok  ' if vide else 'ÉCART'} page {len(simples)} : "
                f"{'blanche' if vide else 'elle porte de l’encre'}"
            )
            echecs += not vide
            return echecs

        if mutant:
            # Le mensonge doit coûter. Un mutant qui passe est une sonde qui ne
            # mesure rien, et c'est un échec du script, pas du moteur.
            if mesurer(1):
                print("pages-simples : mutant refusé, la sonde mord")
                return 0
            print("pages-simples : LE MUTANT PASSE, la sonde ne mesure rien", file=sys.stderr)
            return 1

        echecs = mesurer(0)
        if echecs:
            print(f"pages-simples : {echecs} écart(s)", file=sys.stderr)
            return 1
        if avec_mutant:
            print("  — et le même, l'imposition décalée d'une page :")
            if not mesurer(1):
                print(
                    "pages-simples : LE MUTANT PASSE, la sonde ne mesure rien",
                    file=sys.stderr,
                )
                return 1
            print("  ok   mutant refusé, la sonde mord")

    print(f"pages-simples : {planches} planches, {planches * 2} pages, même livre")
    return 0


if __name__ == "__main__":
    sys.exit(main())
