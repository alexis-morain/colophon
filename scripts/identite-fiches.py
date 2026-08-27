"""Les deux chemins d'une composition rendent-ils le même album ?

L'un part des photos, l'autre des fiches que ces photos ont laissées. S'ils
divergent, c'est que les fiches ne portent pas tout ce que le Composer lit —
ou qu'elles ont vieilli. La comparaison est à l'octet, et elle ne tolère
qu'une exception : `root`, que l'un connaît et l'autre pas.
"""

import pathlib
import sys


def sans_root(chemin):
    """album.json moins la ligne du dossier de photos."""
    lignes = pathlib.Path(chemin).read_bytes().split(b"\n")
    return b"\n".join(l for l in lignes if not l.startswith(b'  "root":'))


def brut(chemin):
    return pathlib.Path(chemin).read_bytes()


def main(photos, fiches, jeu):
    for nom, lire in (("album.json", sans_root), ("curation.json", brut)):
        a, b = lire(f"{photos}/{nom}"), lire(f"{fiches}/{nom}")
        if a == b:
            continue
        la, lb = a.split(b"\n"), b.split(b"\n")
        rang = next(
            (i for i, (x, y) in enumerate(zip(la, lb)) if x != y), min(len(la), len(lb))
        )
        print(
            f"identité {jeu} : {nom} diffère entre les photos et les fiches, "
            f"ligne {rang + 1}",
            file=sys.stderr,
        )
        print(f"  photos : {la[rang:rang + 1]}", file=sys.stderr)
        print(f"  fiches : {lb[rang:rang + 1]}", file=sys.stderr)
        print(
            "  les fiches ne décrivent plus ce que l'analyse mesure : "
            "./scripts/fiches.sh",
            file=sys.stderr,
        )
        return 1
    print(f"identité {jeu} : album.json et curation.json identiques à l'octet")
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
