#!/usr/bin/env python3
"""Le Composer sous et sur le seuil du petit dossier, gardé par le gate.

Les trois jeux de référence font 504, 572 et 575 photos : le chemin du petit
dossier (`pipeline::PETIT_DOSSIER`, 25) et la falaise juste au-dessus n'étaient
gardés par rien, et une régression du Composer sur un dossier de vacances de
vingt photos passait au vert. Ça ne coûte pas une photo de plus : une fiche
tronquée est un fichier JSON tronqué.

    petit-dossier.py <fiche.json> <dossier de sortie>

Compose depuis les N premières photos de la fiche, pour N de part et d'autre
du seuil, et exige : sous le seuil comme dessus, un album écrit et toutes les
photos décodables dedans ou nommées dans `curation.json` ; à deux photos, un
refus nommé, jamais une panique.
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
BIN = RACINE / "target" / "release" / ("colophon.exe" if sys.platform == "win32" else "colophon")
SEUIL = 25


def compose(fiche: dict, n: int, out: Path) -> subprocess.CompletedProcess:
    tronquee = dict(fiche)
    tronquee["photos"] = fiche["photos"][:n]
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    chemin = out / "fiche.json"
    chemin.write_text(json.dumps(tronquee), encoding="utf-8")
    return subprocess.run(
        [str(BIN), "--depuis-fiches", str(chemin), "-o", str(out), "--format", "carre-21"],
        capture_output=True,
        text=True,
    )


def main() -> int:
    fiche = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    base = Path(sys.argv[2])
    for n in (SEUIL - 1, SEUIL, SEUIL + 5):
        out = base / f"n{n}"
        r = compose(fiche, n, out)
        if r.returncode != 0:
            print(f"petit dossier : {n} photos ne composent pas\n{r.stderr}", file=sys.stderr)
            return 1
        album = json.loads((out / "album.json").read_text(encoding="utf-8"))
        posees = {s["src"] for sp in album["spreads"] for s in sp.get("slots", [])}
        curation = json.loads((out / "curation.json").read_text(encoding="utf-8"))
        ecartees = {d["src"] for d in curation}
        attendues = {p["path"] for p in fiche["photos"][:n]}
        manquantes = attendues - posees - ecartees
        if manquantes:
            print(f"petit dossier : à {n}, {len(manquantes)} photos ni posées ni écartées : {sorted(manquantes)[:5]}", file=sys.stderr)
            return 1
        if not posees:
            print(f"petit dossier : à {n}, aucune photo posée", file=sys.stderr)
            return 1
        print(f"petit dossier : {n} photos → {len(posees)} posées, {len(ecartees)} écartées")
    r = compose(fiche, 2, base / "n2")
    if r.returncode == 0 or "panicked" in r.stderr or not r.stderr.strip():
        print(f"petit dossier : deux photos doivent rendre un refus nommé (code {r.returncode})\n{r.stderr}", file=sys.stderr)
        return 1
    print(f"petit dossier : 2 photos → refus nommé ({r.stderr.strip().splitlines()[-1][:80]})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
