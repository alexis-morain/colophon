#!/usr/bin/env python3
"""Ramener un SVG de Commons dans le sous-ensemble du pack d'ornements.

Le pack ne parle que `<svg viewBox>`, `<path d>`, `M L H V C Z` et
`fill-rule` (voir `ornement.rs`). Un fichier de Commons parle le reste :
des `<g transform>`, des `style`, des quadratiques, des arcs, des
métadonnées d'Inkscape. La géométrie est presque toujours déjà là ; c'est
l'emballage qui n'y est pas. Ce script défait l'emballage, et rien d'autre.

Ce qu'il fait :
  - il applique chaque `transform` (translate, scale, rotate, matrix,
    skewX, skewY, composés, sur `<svg>`, `<g>` et `<path>`) aux points ;
  - il rend `S s Q q T t` en cubiques exactes, et `A a` en cubiques par
    arcs de 90° au plus (l'approximation standard, écart sous 0,03 %) ;
  - il retire `style`, `id`, `class`, `xmlns:*`, `fill`, `stroke`, et garde
    `fill-rule="evenodd"` quand il y est ;
  - il aplatit les `<g>` en une suite de `<path>`, en absolu ;
  - il serre le `viewBox` sur l'encre, origine à zéro. Un ornement garde le
    rapport de son `viewBox`, et c'est ce qui fait que sa boîte **est** son
    encre : une marge blanche dans le fichier en serait une dans le prévol.

Ce qu'il refuse, en nommant la chose : une balise qui dessine autrement
qu'un `<path>` (`<rect>`, `<circle>`, `<use>`, `<text>`, une image…), un
tracé en `stroke` sans `fill`, un remplissage clair (un évidement que le
noir unique rendrait plein), une opacité, un chemin caché. **Refuser,
jamais ignorer** : un tracé sauté en silence serait un ornement faux dont
personne ne saurait qu'il l'est.

Il vérifie enfin que le dessin a de l'encre, par une rasterisation à la
ligne de balayage : un fichier qui rend vide après normalisation est un
refus.

Usage :
  scripts/ornement-normaliser.py entree.svg sortie.svg
  scripts/ornement-normaliser.py --test

Python 3 et sa bibliothèque standard, rien d'autre : le script doit tourner
chez un contributeur qui n'a pas le Mac du projet.
"""

import math
import re
import sys
import xml.etree.ElementTree as ET


class Refus(Exception):
    """Ce que le script ne sait pas traduire. Le message est pour un
    contributeur : il nomme l'élément, l'attribut ou la commande."""


# ---------------------------------------------------------------------------
# Les matrices : (a, b, c, d, e, f), comme SVG et le PDF les écrivent.
# ---------------------------------------------------------------------------

IDENTITE = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)


def composer(m, n):
    """m puis n appliquée dedans : le point passe par n d'abord."""
    a, b, c, d, e, f = m
    a2, b2, c2, d2, e2, f2 = n
    return (
        a * a2 + c * b2,
        b * a2 + d * b2,
        a * c2 + c * d2,
        b * c2 + d * d2,
        a * e2 + c * f2 + e,
        b * e2 + d * f2 + f,
    )


def appliquer(m, x, y):
    a, b, c, d, e, f = m
    return (a * x + c * y + e, b * x + d * y + f)


NOMBRE = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"


def lire_transform(t):
    if not re.fullmatch(r"\s*(?:[A-Za-z]+\s*\([^)]*\)[\s,]*)*", t):
        raise Refus(f"transform illisible : « {t} »")
    m = IDENTITE
    for nom, args in re.findall(r"([A-Za-z]+)\s*\(([^)]*)\)", t):
        n = [float(v) for v in re.findall(NOMBRE, args)]
        if nom == "matrix" and len(n) == 6:
            m = composer(m, tuple(n))
        elif nom == "translate" and len(n) in (1, 2):
            m = composer(m, (1, 0, 0, 1, n[0], n[1] if len(n) == 2 else 0.0))
        elif nom == "scale" and len(n) in (1, 2):
            m = composer(m, (n[0], 0, 0, n[1] if len(n) == 2 else n[0], 0, 0))
        elif nom == "rotate" and len(n) in (1, 3):
            r = math.radians(n[0])
            rot = (math.cos(r), math.sin(r), -math.sin(r), math.cos(r), 0, 0)
            if len(n) == 3:
                m = composer(m, (1, 0, 0, 1, n[1], n[2]))
                m = composer(m, rot)
                m = composer(m, (1, 0, 0, 1, -n[1], -n[2]))
            else:
                m = composer(m, rot)
        elif nom == "skewX" and len(n) == 1:
            m = composer(m, (1, 0, math.tan(math.radians(n[0])), 1, 0, 0))
        elif nom == "skewY" and len(n) == 1:
            m = composer(m, (1, math.tan(math.radians(n[0])), 0, 1, 0, 0))
        else:
            raise Refus(f"transform illisible : « {nom}({args}) »")
    return m


# ---------------------------------------------------------------------------
# Les chemins : tout devient M, L, C, Z absolus.
# ---------------------------------------------------------------------------

class Lexeur:
    def __init__(self, d):
        self.d = d
        self.i = 0

    def espaces(self):
        while self.i < len(self.d) and self.d[self.i] in " \t\r\n,":
            self.i += 1

    def fini(self):
        self.espaces()
        return self.i >= len(self.d)

    def commande(self):
        self.espaces()
        if self.i < len(self.d) and self.d[self.i].isalpha():
            c = self.d[self.i]
            if c not in "MmZzLlHhVvCcSsQqTtAa":
                raise Refus(f"commande de chemin inconnue : « {c} »")
            self.i += 1
            return c
        return None

    def nombre(self):
        self.espaces()
        m = re.compile(NOMBRE).match(self.d, self.i)
        if not m:
            raise Refus(f"nombre illisible dans un chemin : « {self.d[self.i:self.i + 12]} »")
        self.i = m.end()
        return float(m.group())

    def drapeau(self):
        self.espaces()
        if self.i < len(self.d) and self.d[self.i] in "01":
            self.i += 1
            return self.d[self.i - 1] == "1"
        raise Refus(f"drapeau d'arc illisible : « {self.d[self.i:self.i + 12]} »")

    def suit_un_nombre(self):
        self.espaces()
        return self.i < len(self.d) and (self.d[self.i].isdigit() or self.d[self.i] in "+-.")


def arc_en_cubiques(x1, y1, rx, ry, phi, grand, sens, x2, y2):
    """Un arc elliptique SVG en cubiques, par la conversion en centre du
    standard SVG (annexe F.6.5), puis des morceaux de 90° au plus."""
    if (x1, y1) == (x2, y2):
        return []
    rx, ry = abs(rx), abs(ry)
    if rx == 0 or ry == 0:
        return [(x1, y1, x2, y2, x2, y2)]
    p = math.radians(phi)
    cp, sp = math.cos(p), math.sin(p)
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    x1p = cp * dx + sp * dy
    y1p = -sp * dx + cp * dy
    lam = (x1p / rx) ** 2 + (y1p / ry) ** 2
    if lam > 1:
        rx *= math.sqrt(lam)
        ry *= math.sqrt(lam)
    num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    co = math.sqrt(max(0.0, num / den)) if den else 0.0
    if grand == sens:
        co = -co
    cxp = co * rx * y1p / ry
    cyp = -co * ry * x1p / rx
    cx = cp * cxp - sp * cyp + (x1 + x2) / 2
    cy = sp * cxp + cp * cyp + (y1 + y2) / 2

    def angle(ux, uy, vx, vy):
        return math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)

    t1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dt = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not sens and dt > 0:
        dt -= 2 * math.pi
    elif sens and dt < 0:
        dt += 2 * math.pi
    n = max(1, math.ceil(abs(dt) / (math.pi / 2) - 1e-9))
    pas = dt / n
    k = 4 / 3 * math.tan(pas / 4)

    def point(t):
        x, y = rx * math.cos(t), ry * math.sin(t)
        return (cp * x - sp * y + cx, sp * x + cp * y + cy)

    def derivee(t):
        x, y = -rx * math.sin(t), ry * math.cos(t)
        return (cp * x - sp * y, sp * x + cp * y)

    sortie = []
    t = t1
    for i in range(n):
        ta, tb = t, t + pas
        pa, pb = point(ta), point(tb)
        da, db = derivee(ta), derivee(tb)
        if i == n - 1:
            pb = (x2, y2)
        sortie.append(
            (pa[0] + k * da[0], pa[1] + k * da[1], pb[0] - k * db[0], pb[1] - k * db[1], pb[0], pb[1])
        )
        t = tb
    return sortie


def lire_chemin(d):
    """Un attribut `d` en segments absolus : ('M', x, y), ('L', x, y),
    ('C', x1, y1, x2, y2, x, y), ('Z',)."""
    lx = Lexeur(d)
    segs = []
    cx = cy = 0.0
    sx = sy = 0.0
    ctrl_c = None  # dernier second point de contrôle d'une cubique
    ctrl_q = None  # dernier point de contrôle d'une quadratique
    cmd = None
    while not lx.fini():
        c = lx.commande()
        if c is None:
            if cmd is None:
                raise Refus("un chemin qui ne commence pas par M ou m")
            if cmd in "Zz":
                raise Refus("un nombre suit un Z sans commande")
            c = cmd
        elif c not in "Mm" and cmd is None:
            raise Refus("un chemin qui ne commence pas par M ou m")
        rel = c.islower()
        C = c.upper()
        ox, oy = (cx, cy) if rel else (0.0, 0.0)
        prochain_ctrl_c = None
        prochain_ctrl_q = None
        if C == "M":
            x, y = lx.nombre() + ox, lx.nombre() + oy
            segs.append(("M", x, y))
            cx, cy, sx, sy = x, y, x, y
            # Une paire qui suit un M est un L, comme le veut SVG.
            cmd = "l" if rel else "L"
            ctrl_c = ctrl_q = None
            continue
        if C == "Z":
            segs.append(("Z",))
            cx, cy = sx, sy
            cmd = c
            ctrl_c = ctrl_q = None
            continue
        if C == "L":
            x, y = lx.nombre() + ox, lx.nombre() + oy
            segs.append(("L", x, y))
        elif C == "H":
            x, y = lx.nombre() + ox, cy
            segs.append(("L", x, y))
        elif C == "V":
            x, y = cx, lx.nombre() + (cy if rel else 0.0)
            segs.append(("L", x, y))
        elif C == "C":
            x1, y1 = lx.nombre() + ox, lx.nombre() + oy
            x2, y2 = lx.nombre() + ox, lx.nombre() + oy
            x, y = lx.nombre() + ox, lx.nombre() + oy
            segs.append(("C", x1, y1, x2, y2, x, y))
            prochain_ctrl_c = (x2, y2)
        elif C == "S":
            x1, y1 = (2 * cx - ctrl_c[0], 2 * cy - ctrl_c[1]) if ctrl_c else (cx, cy)
            x2, y2 = lx.nombre() + ox, lx.nombre() + oy
            x, y = lx.nombre() + ox, lx.nombre() + oy
            segs.append(("C", x1, y1, x2, y2, x, y))
            prochain_ctrl_c = (x2, y2)
        elif C in "QT":
            if C == "Q":
                qx, qy = lx.nombre() + ox, lx.nombre() + oy
            else:
                qx, qy = (2 * cx - ctrl_q[0], 2 * cy - ctrl_q[1]) if ctrl_q else (cx, cy)
            x, y = lx.nombre() + ox, lx.nombre() + oy
            # Une quadratique est une cubique dont les contrôles sont aux
            # deux tiers : exacte, pas une approximation.
            segs.append(
                ("C", cx + 2 / 3 * (qx - cx), cy + 2 / 3 * (qy - cy), x + 2 / 3 * (qx - x), y + 2 / 3 * (qy - y), x, y)
            )
            prochain_ctrl_q = (qx, qy)
        elif C == "A":
            rx, ry, phi = lx.nombre(), lx.nombre(), lx.nombre()
            grand, sens = lx.drapeau(), lx.drapeau()
            x, y = lx.nombre() + ox, lx.nombre() + oy
            for cub in arc_en_cubiques(cx, cy, rx, ry, phi, grand, sens, x, y):
                segs.append(("C",) + cub)
        cx, cy = x, y
        ctrl_c, ctrl_q = prochain_ctrl_c, prochain_ctrl_q
        cmd = c
    return segs


def transformer(segs, m):
    sortie = []
    for s in segs:
        if s[0] == "Z":
            sortie.append(s)
            continue
        pts = []
        for i in range(1, len(s), 2):
            pts.extend(appliquer(m, s[i], s[i + 1]))
        sortie.append((s[0],) + tuple(pts))
    return sortie


# ---------------------------------------------------------------------------
# Le document.
# ---------------------------------------------------------------------------

SVG_NS = "http://www.w3.org/2000/svg"

# Ce qui ne dessine rien dans l'espace SVG : les métadonnées et les titres.
# Une balise d'un autre espace (Inkscape, Sodipodi, RDF, Dublin Core) ne
# dessine chez aucun lecteur, par définition, et se saute pour la même
# raison. Toute autre balise inconnue est un refus.
SANS_ENCRE = {"metadata", "title", "desc"}

# Les propriétés qui peignent, héritées d'un `<g>` à ses enfants.
# `opacity` ne s'hérite pas en SVG, elle se compose ; la garder dans
# l'héritage suffit à la refuser où qu'elle soit posée.
HERITEES = ("fill", "stroke", "fill-rule", "display", "visibility", "opacity", "fill-opacity")


def local(tag):
    return tag.split("}", 1)[1] if "}" in tag else tag


def hors_svg(el):
    return isinstance(el.tag, str) and el.tag.startswith("{") and not el.tag.startswith("{" + SVG_NS + "}")


def declarations(texte):
    p = {}
    for decl in texte.split(";"):
        if ":" in decl:
            k, v = decl.split(":", 1)
            if k.strip() in HERITEES:
                p[k.strip()] = v.strip()
    return p


def feuille(racine):
    """Les règles `.classe { … }` des `<style>` du document. Un sélecteur
    plus riche qu'une classe est un refus : le lire à moitié serait peindre
    un chemin que le fichier ne peint pas."""
    classes = {}
    for el in racine.iter():
        if not isinstance(el.tag, str) or local(el.tag) != "style" or hors_svg(el):
            continue
        texte = re.sub(r"/\*.*?\*/", "", el.text or "", flags=re.S)
        for selecteurs, corps in re.findall(r"([^{}]+)\{([^{}]*)\}", texte):
            for sel in selecteurs.split(","):
                sel = sel.strip()
                if not re.fullmatch(r"\.[A-Za-z_][\w-]*", sel):
                    raise Refus(f"sélecteur CSS « {sel} » dans <style> : seules les classes se lisent")
                classes.setdefault(sel[1:], {}).update(declarations(corps))
        if re.sub(r"([^{}]+)\{([^{}]*)\}", "", texte).strip():
            raise Refus("<style> illisible : seules des règles « .classe { … } » se lisent")
    return classes


def declarations_brutes(el):
    """Les propriétés du `style` d'un élément, toutes, pour y chercher ce
    qu'on refuse (un `clip-path` peut s'écrire en style)."""
    return {d.split(":", 1)[0].strip() for d in (el.get("style") or "").split(";") if ":" in d}


def proprietes(el, parent, classes):
    """Présentation, puis classes, puis `style` : l'ordre de la cascade SVG."""
    p = dict(parent)
    for k in HERITEES:
        if el.get(k) is not None:
            p[k] = el.get(k).strip()
    for c in (el.get("class") or "").split():
        p.update(classes.get(c, {}))
    p.update(declarations(el.get("style") or ""))
    return p


def clair(couleur):
    """Vrai pour un remplissage clair : blanc, ou proche. Le pack n'a qu'un
    noir, donc un évidement peint en blanc deviendrait une tache."""
    c = couleur.lower().strip()
    nommes = {"white": "#ffffff", "black": "#000000"}
    c = nommes.get(c, c)
    m = re.fullmatch(r"#([0-9a-f]{3}|[0-9a-f]{6})", c)
    if m:
        h = m.group(1)
        if len(h) == 3:
            h = "".join(ch * 2 for ch in h)
        r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    else:
        m = re.fullmatch(r"rgb\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)\s*\)", c)
        if not m:
            return False
        r, g, b = (int(v) for v in m.groups())
    return 0.2126 * r + 0.7152 * g + 0.0722 * b > 127


AVERTISSEMENTS = set()


def peint(p, balise):
    """Décide si un `<path>` entre, et avec quelle règle. Refuse ce que le
    noir unique du pack ne saurait pas rendre."""
    if p.get("display") == "none" or p.get("visibility") in ("hidden", "collapse"):
        raise Refus(f"<{balise}> caché (display ou visibility) : le retirer avant, ou l'afficher")
    for k in ("opacity", "fill-opacity"):
        if k in p:
            try:
                v = float(p[k])
            except ValueError:
                raise Refus(f"{k} illisible sur <{balise}> : « {p[k]} »")
            if v < 1:
                raise Refus(f"{k}={p[k]} sur <{balise}> : le pack n'a qu'un noir plein")
    fill = p.get("fill", "black")
    stroke = p.get("stroke", "none")
    if fill == "none":
        if stroke != "none":
            raise Refus(f"<{balise}> en stroke sans fill : convertir le contour en chemin (Inkscape : contour en chemin)")
        raise Refus(f"<{balise}> sans fill ni stroke : il ne dessine rien")
    if fill.startswith("url("):
        raise Refus(f"<{balise}> rempli par {fill} : un dégradé ou un motif n'entre pas")
    if clair(fill):
        raise Refus(f"<{balise}> rempli en clair ({fill}) : un évidement peint deviendrait noir")
    if stroke != "none":
        # Un contour sur un remplissage épaissit le dessin d'une demi-épaisseur
        # de trait. Le pack n'a pas de contour : on le retire, comme la
        # décision de C-s1 le veut, mais on le dit, pour que l'œil vérifie.
        AVERTISSEMENTS.add(f"contour {stroke} retiré d'un <{balise}> rempli")
    return p.get("fill-rule") == "evenodd"


def normaliser(src):
    """Un SVG en (viewBox, [(segments, evenodd)]), serré sur l'encre."""
    try:
        racine = ET.fromstring(src)
    except ET.ParseError as e:
        raise Refus(f"XML illisible : {e}")
    if local(racine.tag) != "svg":
        raise Refus(f"la racine est <{local(racine.tag)}>, pas <svg>")
    classes = feuille(racine)
    chemins = []

    def parcourir(el, m, props, profondeur):
        if not isinstance(el.tag, str) or hors_svg(el):
            return
        nom = local(el.tag)
        if nom in SANS_ENCRE or nom == "style":
            return
        if nom == "defs":
            # Rien de ce qu'une `<defs>` porte ne se dessine de soi-même : il
            # faut un `<use>`, un `clip-path`, un `mask`, un `fill="url()"` ou
            # un marqueur pour l'appeler, et chacun est refusé là où il appelle.
            return
        if nom == "svg" and profondeur > 0:
            raise Refus("un <svg> dans un <svg> : aplatir avant de déposer")
        if nom not in ("svg", "g", "path"):
            raise Refus(f"balise <{nom}> hors du sous-ensemble : convertir en <path> (Inkscape : objet en chemin)")
        p = proprietes(el, props, classes)
        for attr in ("clip-path", "mask", "filter", "marker-start", "marker-mid", "marker-end", "marker"):
            if el.get(attr) is not None or attr in declarations_brutes(el):
                raise Refus(f"{attr} sur <{nom}> : un découpage, un masque, un filtre ou un marqueur n'entre pas")
        if el.get("transform"):
            m = composer(m, lire_transform(el.get("transform")))
        if nom == "path":
            d = el.get("d") or ""
            segs = lire_chemin(d)
            if not any(s[0] in "LC" for s in segs):
                raise Refus("un <path> qui ne dessine rien")
            evenodd = peint(p, nom)
            segs = transformer(segs, m)
            # Un trait `M0,3 H15` rempli et contouré ne dessine que par son
            # contour : son remplissage n'a pas d'aire. Retirer le contour
            # laisserait une boîte plus grande que l'encre, le seul mensonge
            # que le pack ne tolère pas.
            x0, y0, x1, y1 = boite([(segs, False)])
            if sum(abs(aire_poly(q)) for q in aplatir(segs)) <= 1e-6 * max(x1 - x0, y1 - y0) ** 2:
                raise Refus(f"un <{nom}> sans aire : c'est son contour qui dessine, et le pack n'en a pas")
            chemins.append((segs, evenodd))
            return
        for enfant in el:
            parcourir(enfant, m, p, profondeur + 1)

    parcourir(racine, IDENTITE, {}, 0)
    if not chemins:
        raise Refus("aucun <path> dans le fichier")
    return serrer(chemins)


# ---------------------------------------------------------------------------
# La boîte de l'encre, et la mise à l'origine.
# ---------------------------------------------------------------------------

def extremes_cubique(p0, p1, p2, p3):
    """Les valeurs de t dans ]0,1[ où une coordonnée de la cubique atteint
    un extremum : les racines de sa dérivée, un trinôme."""
    a = -p0 + 3 * p1 - 3 * p2 + p3
    b = 2 * (p0 - 2 * p1 + p2)
    c = p1 - p0
    ts = []
    if abs(a) < 1e-12:
        if abs(b) > 1e-12:
            ts.append(-c / b)
    else:
        disc = b * b - 4 * a * c
        if disc >= 0:
            r = math.sqrt(disc)
            ts += [(-b + r) / (2 * a), (-b - r) / (2 * a)]
    return [t for t in ts if 0 < t < 1]


def bezier(p0, p1, p2, p3, t):
    u = 1 - t
    return u * u * u * p0 + 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t * p3


def boite(chemins):
    xs, ys = [], []
    for segs, _ in chemins:
        cx = cy = 0.0
        for s in segs:
            if s[0] in "ML":
                cx, cy = s[1], s[2]
                xs.append(cx)
                ys.append(cy)
            elif s[0] == "C":
                for pts, acc in (((cx, s[1], s[3], s[5]), xs), ((cy, s[2], s[4], s[6]), ys)):
                    acc.append(pts[3])
                    for t in extremes_cubique(*pts):
                        acc.append(bezier(*pts, t))
                cx, cy = s[5], s[6]
    return min(xs), min(ys), max(xs), max(ys)


def serrer(chemins):
    x0, y0, x1, y1 = boite(chemins)
    w, h = x1 - x0, y1 - y0
    if w <= 0 or h <= 0:
        raise Refus("un dessin sans largeur ou sans hauteur")
    decalage = (1, 0, 0, 1, -x0, -y0)
    return [w, h], [(transformer(segs, decalage), eo) for segs, eo in chemins]


# ---------------------------------------------------------------------------
# L'écriture.
# ---------------------------------------------------------------------------

def nombre(v, dec):
    s = f"{v:.{dec}f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def ecrire(taille, chemins):
    w, h = taille
    # Une précision relative : un dix-millième de la plus grande dimension,
    # bien en dessous du pas d'une imprimante sur une page entière.
    dec = max(0, 4 - math.floor(math.log10(max(w, h))))
    corps = []
    for segs, eo in chemins:
        d = []
        for s in segs:
            if s[0] == "Z":
                d.append("Z")
            else:
                d.append(s[0] + " ".join(nombre(v, dec) for v in s[1:]))
        attr = ' fill-rule="evenodd"' if eo else ""
        corps.append(f'<path{attr} d="{"".join(d)}"/>')
    vb = f"0 0 {nombre(w, dec)} {nombre(h, dec)}"
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}">' + "".join(corps) + "</svg>\n"


# ---------------------------------------------------------------------------
# L'encre : une rasterisation à la ligne de balayage, règle de remplissage
# comprise. Pas un rendu fidèle, une réponse à « y a-t-il de l'encre ».
# ---------------------------------------------------------------------------

def aplatir(segs, pas=16):
    """Les sous-chemins en polygones fermés."""
    polys, courant = [], []
    cx = cy = 0.0
    for s in segs:
        if s[0] == "M":
            if len(courant) > 1:
                polys.append(courant)
            cx, cy = s[1], s[2]
            courant = [(cx, cy)]
        elif s[0] == "L":
            cx, cy = s[1], s[2]
            courant.append((cx, cy))
        elif s[0] == "C":
            for i in range(1, pas + 1):
                t = i / pas
                courant.append((bezier(cx, s[1], s[3], s[5], t), bezier(cy, s[2], s[4], s[6], t)))
            cx, cy = s[5], s[6]
        elif s[0] == "Z":
            if len(courant) > 1:
                polys.append(courant)
            courant = [courant[0]] if courant else []
            if courant:
                cx, cy = courant[0]
    if len(courant) > 1:
        polys.append(courant)
    return polys


def raster(taille, chemins, cote=200):
    """Une grille de booléens, `cote` pixels sur le grand côté."""
    w, h = taille
    e = cote / max(w, h)
    W, H = max(1, round(w * e)), max(1, round(h * e))
    grille = [[False] * W for _ in range(H)]
    for segs, eo in chemins:
        aretes = []
        for poly in aplatir(segs):
            n = len(poly)
            for i in range(n):
                (xa, ya), (xb, yb) = poly[i], poly[(i + 1) % n]
                if ya != yb:
                    aretes.append((xa * e, ya * e, xb * e, yb * e))
        for j in range(H):
            y = j + 0.5
            croisements = []
            for xa, ya, xb, yb in aretes:
                if (ya <= y < yb) or (yb <= y < ya):
                    x = xa + (y - ya) * (xb - xa) / (yb - ya)
                    croisements.append((x, 1 if yb > ya else -1))
            croisements.sort()
            enroulement = 0
            for k in range(len(croisements) - 1):
                enroulement += croisements[k][1]
                dedans = (enroulement % 2 != 0) if eo else (enroulement != 0)
                if dedans:
                    xa, xb = croisements[k][0], croisements[k + 1][0]
                    for i in range(max(0, math.ceil(xa - 0.5)), min(W, math.ceil(xb - 0.5))):
                        grille[j][i] = True
    return grille


def encre(taille, chemins):
    g = raster(taille, chemins)
    total = sum(len(l) for l in g)
    return sum(sum(l) for l in g) / total


def aire_poly(poly):
    """L'aire signée d'un polygone, par la formule du lacet."""
    s = 0.0
    n = len(poly)
    for i in range(n):
        (xa, ya), (xb, yb) = poly[i], poly[(i + 1) % n]
        s += xa * yb - xb * ya
    return s / 2


def aire(segs):
    """L'aire signée d'un chemin, sur son aplatissement fin."""
    return sum(aire_poly(q) for q in aplatir(segs, pas=64))


# ---------------------------------------------------------------------------
# Les tests : `--test`.
# ---------------------------------------------------------------------------

def tests():
    def premier(svg):
        return normaliser(svg)[1][0][0]

    # translate : la boîte se serre sur l'encre, donc on lit le décalage
    # sur un second chemin, resté à sa place relative.
    t = normaliser(
        '<svg viewBox="0 0 100 100"><path d="M0,0 H10 V10 H0 Z"/>'
        '<g transform="translate(20,5)"><path d="M0,0 H10 V10 H0 Z"/></g></svg>'
    )
    assert t[0] == [30, 15], t[0]
    assert t[1][1][0][0] == ("M", 20, 5), t[1][1][0][0]

    # matrix, composée avec un translate et un scale : l'ordre compte.
    s = normaliser(
        '<svg viewBox="0 0 9 9"><path d="M0,0 L1,0 L1,1 Z"/>'
        '<g transform="translate(10,0) scale(2)"><path transform="matrix(0,1,-1,0,0,0)" d="M0,0 L3,0 L3,1 Z"/></g></svg>'
    )
    # (3,0) tourné d'un quart de tour devient (0,3), doublé (0,6), décalé (10,6).
    # La boîte part de x=0 (premier chemin) et y=0.
    pts = s[1][1][0]
    assert pts[1] == ("L", 10, 6), pts
    assert pts[2] == ("L", 8, 6), pts

    # Q → C, exacte : les contrôles sont aux deux tiers.
    q = lire_chemin("M0,0 Q3,3 6,0 T12,0")
    assert q[1] == ("C", 2, 2, 4, 2, 6, 0), q[1]
    assert all(abs(a - b) < 1e-9 for a, b in zip(q[2][1:], (8, -2, 10, -2, 12, 0))), q[2]

    # S reprend le reflet du second contrôle précédent.
    sc = lire_chemin("M0,0 C1,1 2,1 3,0 s2,-1 3,0")
    assert sc[2] == ("C", 4, -1, 5, -1, 6, 0), sc[2]

    # A → C sur un cercle : l'aire tient à 1 %.
    r = 10
    cercle = premier(
        f'<svg viewBox="0 0 20 20"><path d="M0,10 A{r},{r} 0 0 1 20,10 A{r},{r} 0 0 1 0,10 Z"/></svg>'
    )
    assert all(s[0] in "MCZ" for s in cercle), cercle
    a = abs(aire(cercle))
    assert abs(a - math.pi * r * r) / (math.pi * r * r) < 0.01, a
    # Et les drapeaux collés se lisent.
    assert len(lire_chemin("M0,0 a5,5 0 0110,0")) == 3

    # Refus : un <text>, nommé.
    for svg, mot in [
        ('<svg viewBox="0 0 9 9"><text>non</text></svg>', "<text>"),
        ('<svg viewBox="0 0 9 9"><circle r="3"/></svg>', "<circle>"),
        ('<svg viewBox="0 0 9 9"><rect width="3" height="3"/></svg>', "<rect>"),
        ('<svg viewBox="0 0 9 9"><use href="#a"/></svg>', "<use>"),
        ('<svg viewBox="0 0 9 9"><path fill="none" stroke="#000" d="M0,0 L5,5"/></svg>', "stroke sans fill"),
        ('<svg viewBox="0 0 9 9"><path fill="#fff" d="M0,0 L5,5 L0,5 Z"/></svg>', "clair"),
        ('<svg viewBox="0 0 9 9"><path stroke="#000" d="M0,3 H15"/></svg>', "sans aire"),
    ]:
        try:
            normaliser(svg)
        except Refus as e:
            assert mot in str(e), (mot, str(e))
        else:
            raise AssertionError(f"{mot} aurait dû être refusé")

    # La sortie parle le sous-ensemble, et un carré a de l'encre.
    sortie = ecrire(*normaliser('<svg viewBox="0 0 9 9"><path style="fill:#123" id="x" d="M1,1 h4 v4 h-4 z"/></svg>'))
    assert sortie == '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><path d="M0 0L4 0L4 4L0 4Z"/></svg>\n', sortie
    assert encre([4, 4], [(lire_chemin("M0 0L4 0L4 4L0 4Z"), False)]) == 1.0
    print("ornement-normaliser : tests ok")


def main(argv):
    if argv[1:] == ["--test"]:
        tests()
        return 0
    if len(argv) != 3:
        print(__doc__.split("Usage :")[1].split("\n\n")[0], file=sys.stderr)
        return 2
    entree, sortie = argv[1], argv[2]
    try:
        with open(entree, encoding="utf-8") as f:
            taille, chemins = normaliser(f.read())
        part = encre(taille, chemins)
        if part == 0:
            raise Refus("le dessin rend vide après normalisation")
    except Refus as e:
        print(f"{entree} : refusé : {e}", file=sys.stderr)
        return 1
    for a in sorted(AVERTISSEMENTS):
        print(f"{entree} : avertit : {a}", file=sys.stderr)
    texte = ecrire(taille, chemins)
    with open(sortie, "w", encoding="utf-8") as f:
        f.write(texte)
    print(f"{sortie} : {len(chemins)} chemins, {len(texte)} octets, viewBox {texte.split('viewBox=')[1].split('>')[0]}, encre {part:.1%}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
