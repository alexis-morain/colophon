#!/usr/bin/env python3
"""La version vit à quatre endroits, et ce script est le seul qui les compare.

`Cargo.toml` (le workspace), `package.json`, `package-lock.json` (deux fois)
et `tauri.conf.json` portent chacun la version. `release.yml` prend la sienne
du tag seul : taguer sans avoir monté les quatre donnait un DMG nommé 1.0.0,
une app installée qui se déclare 0.9.0 et un `latest.json` qui annonce 1.0.0
— donc une mise à jour disponible en permanence, que l'installer ne fait
jamais disparaître. Rien ne les comparait.

Sans argument : les quatre doivent être identiques (le gate). Avec
`--attendue X` : elles doivent en plus valoir X (la release, X venant du tag).
"""
import json
import re
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
APP = RACINE / "crates" / "colophon-app"


def lues() -> dict[str, str]:
    cargo = (RACINE / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"', cargo, re.M | re.S)
    if not m:
        m = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.M)
    pkg = json.loads((APP / "package.json").read_text(encoding="utf-8"))
    lock = json.loads((APP / "package-lock.json").read_text(encoding="utf-8"))
    tauri = json.loads((APP / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    return {
        "Cargo.toml": m.group(1) if m else "?",
        "package.json": pkg.get("version", "?"),
        "package-lock.json": lock.get("version", "?"),
        'package-lock.json (packages[""])': lock.get("packages", {}).get("", {}).get("version", "?"),
        "tauri.conf.json": tauri.get("version", "?"),
    }


def main() -> int:
    attendue = None
    if len(sys.argv) == 3 and sys.argv[1] == "--attendue":
        attendue = sys.argv[2]
    elif len(sys.argv) != 1:
        print("usage : versions.py [--attendue X]", file=sys.stderr)
        return 2
    v = lues()
    distinctes = sorted(set(v.values()))
    ok = len(distinctes) == 1 and (attendue is None or distinctes[0] == attendue)
    if ok:
        print(f"versions : {distinctes[0]} aux {len(v)} endroits")
        return 0
    print("versions : elles divergent, et une release les comparerait au tag seul", file=sys.stderr)
    for nom, val in v.items():
        print(f"  {nom} : {val}", file=sys.stderr)
    if attendue is not None:
        print(f"  attendue (le tag) : {attendue}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
