// Which renderer draws a spread: the DOM one, or the canvas.
//
// **Why a setting and not a build flag.** Wave 2.5 has to measure the two
// against each other on one machine, in one afternoon, under one load — and
// it has to do it in an installed bundle, because a dev server is not what
// anyone runs. A switch somebody can flip without recompiling is the whole
// point; an environment variable would mean two builds and a comparison
// between two different binaries.
//
// **The default is `dom` until the gestures are proven**, and it moves to
// `canvas` in a commit that does nothing else, so the day the picture
// changes is a line in the log rather than a side effect. Until 2.5 has
// decided, neither renderer is allowed to rot: both consume the same scene,
// and the parity test runs on the scene, not on the DOM.
//
// Same shape as `i18n.ts`, and for the same reason: a store, a hook, one
// key in `localStorage`, no library.

import { useSyncExternalStore } from "react";

export type Rendu = "dom" | "canvas";

const CLE = "colophon.rendu";

let courant: Rendu = (() => {
  try {
    const garde =
      typeof localStorage === "undefined" ? null : localStorage.getItem(CLE);
    if (garde === "dom" || garde === "canvas") return garde;
  } catch {
    /* un stockage bloqué ne coûte que la mémoire du choix */
  }
  return "dom";
})();

const abonnes = new Set<() => void>();

export function rendu(): Rendu {
  return courant;
}

export function setRendu(r: Rendu): void {
  if (r === courant) return;
  courant = r;
  try {
    if (typeof localStorage !== "undefined") localStorage.setItem(CLE, r);
  } catch {
    /* idem */
  }
  abonnes.forEach((f) => f());
}

/** Rend le composant appelant sensible au changement de rendu. */
export function useRendu(): Rendu {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => courant,
    () => courant,
  );
}
