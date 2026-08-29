// The engine's geometry dump, held once per album: every rectangle, anchor
// and type constant the editor draws comes from here, produced by the same
// arithmetic that writes the PDF (`gabarit::catalogue` and `pdf.rs`). The
// editor declares no dimension of its own: what used to be a 350-line port
// in album.ts is now a lookup.
//
// Two stores: the open album's dump, loaded before the album is adopted,
// and a per-format cache for the creation screen, which previews page
// formats before any album exists.

export type DumpTemplate = {
  /** Full-capacity slot rectangles, origin bottom-up like the PDF. */
  slots: number[][];
  /** Caption anchor at full capacity (kept for the parity diff). */
  caption: number[];
  /** Caption anchor per photo count, index = count, 0..=capacity. */
  captions: number[][];
  /** Signed caption height, mm: positive a band under the frame, negative
   *  an overlay on the photo, zero the free-spot hunt. The anchors above
   *  already account for it; the sign travels for the picker and badges. */
  legende: number;
};

export type Dump = {
  trim_mm: { w: number; h: number };
  bleed_mm: number;
  media: { w: number; h: number; margin: number; gutter: number };
  /** `[name, capacity]` in catalogue order: a JSON map sorts its keys. */
  ordre: [string, number][];
  templates: Record<string, DumpTemplate>;
  fallbacks: Record<string, [string, number]>;
  anchors: {
    texte: [number, number];
    colophon: [number, number];
    garde: [number, number];
    garde_place: number;
  };
  constantes: Record<string, number>;
  garde_samples: {
    texte: string;
    place: number;
    lignes: [string, number, number][];
  }[];
  crop_windows?: {
    rect: [number, number];
    image: [number, number];
    focal: [number, number];
    zoom: number;
    window: [number, number, number, number];
  }[];
  covers?: {
    profil: string;
    spreads: number;
    sheet: [number, number];
    spine: number;
    /** `[x, width]` of the back panel then the front one. */
    panels: [[number, number], [number, number]];
  }[];
};

let courant: Dump | null = null;
const parFormat = new Map<string, Dump>();
const hooks: ((d: Dump) => void)[] = [];

const cle = (w: number, h: number, bleed: number) => `${w}x${h}/${bleed}`;

/** Runs the hook on every album dump installed from now on (and right away
 *  when one is already here). `album.ts` rehydrates its constants with it;
 *  registering here rather than importing the other way keeps the two
 *  modules from circling. */
export function onGeometrie(hook: (d: Dump) => void): void {
  hooks.push(hook);
  if (courant) hook(courant);
}

/** Install the open album's dump. Called by the bridge before the album
 *  reaches React: nothing below renders without it. */
export function setGeometrie(d: Dump): void {
  courant = d;
  parFormat.set(cle(d.trim_mm.w, d.trim_mm.h, d.bleed_mm), d);
  hooks.forEach((h) => h(d));
}

/** Install a bare format's dump (creation screen previews). */
export function setGeometrieFormat(d: Dump): void {
  parFormat.set(cle(d.trim_mm.w, d.trim_mm.h, d.bleed_mm), d);
}

/**
 * The dump for a trim size, the open album's first. Throwing is right: a
 * caller that asks for a geometry nobody loaded is a bug, and a silently
 * wrong rectangle would reach the screen where a thrown line reaches a log.
 */
export function geometrie(trim: { w: number; h: number }, bleed: number): Dump {
  if (
    courant &&
    courant.trim_mm.w === trim.w &&
    courant.trim_mm.h === trim.h &&
    courant.bleed_mm === bleed
  ) {
    return courant;
  }
  const d = parFormat.get(cle(trim.w, trim.h, bleed));
  if (!d) {
    throw new Error(
      `géométrie non chargée pour ${trim.w} × ${trim.h} mm (fond perdu ${bleed})`,
    );
  }
  return d;
}

/** The open album's dump, whatever its format. */
export function geometrieCourante(): Dump {
  if (!courant) throw new Error("géométrie non chargée : aucun album ouvert");
  return courant;
}

export function geometrieChargee(): boolean {
  return courant !== null;
}

/** A bare format's dump when it is already here, null while it loads. */
export function geometrieFormatSync(
  trim: { w: number; h: number },
  bleed: number,
): Dump | null {
  return parFormat.get(cle(trim.w, trim.h, bleed)) ?? null;
}

/**
 * Point the current dump at a format already cached, and say whether it was.
 *
 * A format switch changes the album's trim under a fully rendered editor:
 * the dump loaded when the album opened describes the *old* page, and the
 * lookup above throws for the new one — which, with no error boundary over
 * the tree, is a white window and nothing else. So the switch loads the
 * target's dump before it applies (`chargeGeometrieFormat`), then hands it
 * over here. `false` means nobody loaded it, and the caller must not apply.
 *
 * The old dump stays in the cache, which is what makes ⌘Z on a format
 * switch draw the old page rather than throw for it in turn.
 */
export function adopterGeometrie(
  trim: { w: number; h: number },
  bleed: number,
): boolean {
  const d = parFormat.get(cle(trim.w, trim.h, bleed));
  if (!d) return false;
  courant = d;
  hooks.forEach((h) => h(d));
  return true;
}
