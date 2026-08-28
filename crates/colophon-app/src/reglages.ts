// The open album's adjustments, mirrored for the seven drawing surfaces,
// plus the draft of the slider being dragged. Same shape as `rendu.ts`: a
// store, a hook, no library.
//
// **App remains the single truth.** It re-poses the whole table here on
// every album change — opening, an edit, ⌘Z, a bascule, a recomposition —
// and this store persists nothing and decides nothing. The draft is the one
// thing that lives here alone: while a slider moves, only the draft moves,
// and the committed step goes through `edits.ts::setReglage` at release. A
// component writing a committed réglage here instead would make ⌘Z lie,
// and it would be a bug.

import { useSyncExternalStore } from "react";
import type { Reglage } from "./album";
import { filtreCss } from "./reglage";

let table: Record<string, Reglage> = {};
let brouillon: { src: string; reglage: Reglage } | null = null;

const abonnes = new Set<() => void>();
/** Bumped on every change: the hook's snapshot, cheap to compare. */
let version = 0;

function prevenir(): void {
  version++;
  abonnes.forEach((f) => f());
}

/** App re-poses the album's whole table after any album change. */
export function poserReglages(t: Record<string, Reglage> | undefined): void {
  table = t ?? {};
  brouillon = null;
  prevenir();
}

/** The slider's draft: only the store moves while the gesture runs. Null
 *  clears it (commit or abandon). */
export function poserBrouillon(src: string, reglage: Reglage | null): void {
  brouillon = reglage === null ? null : { src, reglage };
  prevenir();
}

/** What a surface should draw for one photo: the draft while its slider
 *  moves, the album's table otherwise, undefined for « no adjustment ». */
export function reglageDe(src: string): Reglage | undefined {
  if (brouillon && brouillon.src === src) return brouillon.reglage;
  return table[src];
}

/** The committed adjustment only, draft excluded: what the canvas fallback
 *  burns into its bitmap, so a drag without `ctx.filter` costs nothing and
 *  the case follows at release. */
export function reglagePose(src: string): Reglage | undefined {
  return table[src];
}

/** The CSS filter chain of one photo, for the six `<img>` surfaces and the
 *  canvas's `ctx.filter`. One function, so no surface can disagree. */
export function filtreDe(src: string): string | undefined {
  return filtreCss(reglageDe(src));
}

/** Runs whenever the table or the draft changes: the canvas repaints on
 *  this, exactly as it repaints on `surImage`. */
export function surReglage(hook: () => void): () => void {
  abonnes.add(hook);
  return () => {
    abonnes.delete(hook);
  };
}

/** Makes the calling component re-render on any adjustment change, so its
 *  `filtreDe`/`reglageDe` reads are never stale. */
export function useReglages(): number {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => version,
    () => version,
  );
}
