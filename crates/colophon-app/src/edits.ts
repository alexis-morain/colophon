// Edit operations on an album. All pure: each returns a fresh Album, which is
// what the undo stack stores. The template fallback rule lives in core
// (`pdf.rs::fallback_template`); album.ts carries its port.

import {
  Album,
  COLOPHON_TEMPLATE,
  Cover,
  Discard,
  Slot,
  Spread,
  TEMPLATES,
  ZOOM_MAX,
  ZOOM_MIN,
  fallbackTemplate,
  templateCapacity,
  templateForCount,
} from "./album";

function withSpread(album: Album, at: number, spread: Spread | null): Album {
  const spreads = album.spreads.slice();
  if (spread) spreads[at] = spread;
  else spreads.splice(at, 1);
  return { ...album, spreads };
}

/** Every hand edit stamps its spread: the badge, and the recomposition
 *  shield, derive from this single mark. */
function touched(spread: Spread): Spread {
  return spread.edited ? spread : { ...spread, edited: true };
}

/**
 * Remove one photo from a spread. The template follows what remains: a quad
 * losing a photo becomes a trio. Counts without an exact template drop to the
 * largest one below, so the spread may lose a tail photo too; undo restores
 * everything. An emptied spread leaves the album.
 */
export function removePhoto(album: Album, at: number, slot: number): Album {
  const spread = album.spreads[at];
  if (!spread || slot >= spread.slots.length) return album;
  const slots = spread.slots.filter((_, i) => i !== slot);
  const fb = fallbackTemplate(spread.template, slots.length);
  if (!fb) return withSpread(album, at, null);
  return withSpread(album, at, touched({
    ...spread,
    template: fb.template,
    slots: slots.slice(0, fb.capacity),
  }));
}

/**
 * Give a spread another template. A smaller one drops the tail photos; the
 * picker only offers templates the current count can fill, so no grid ever
 * shows a hole.
 */
export function changeTemplate(album: Album, at: number, template: string): Album {
  const spread = album.spreads[at];
  if (!spread || spread.template === template) return album;
  const cap = templateCapacity(template);
  if (cap > spread.slots.length) return album;
  return withSpread(album, at, touched({
    ...spread,
    template,
    slots: spread.slots.slice(0, cap),
  }));
}

/**
 * A spread can absorb one more photo only when the count n+1 has an exact
 * template; otherwise its grid would render with a hole. Same core table as
 * the fallback, read upward.
 */
export function growTemplate(
  current: string,
  count: number,
): { template: string; capacity: number } | null {
  const fam = templateForCount(count + 1);
  if (!fam || fam[1] !== count + 1) return null;
  const [family, capacity] = fam;
  const verso = `${family}_verso`;
  const keepVerso =
    current.endsWith("_verso") && TEMPLATES.some(([t]) => t === verso);
  return { template: keepVerso ? verso : family, capacity };
}

/**
 * Why a photo cannot move from one spread to another, or null when it can.
 * Moving never sacrifices a third photo: when the source would fall past an
 * exact template (6→5, 8→7), the move is refused rather than dropping a
 * bystander.
 */
export function moveBlocker(
  album: Album,
  from: number,
  slot: number,
  to: number,
): "no_target" | "target_full" | "source_breaks" | null {
  const src = album.spreads[from];
  const dst = album.spreads[to];
  if (!src || !dst || from === to || !src.slots[slot]) return "no_target";
  if (!growTemplate(dst.template, dst.slots.length)) return "target_full";
  const rest = src.slots.length - 1;
  if (rest > 0) {
    const fb = fallbackTemplate(src.template, rest);
    if (!fb || fb.capacity !== rest) return "source_breaks";
  }
  return null;
}

/**
 * Move one photo to another spread, appended at the end. The source falls
 * back one template (or leaves the album when emptied); the target grows one.
 */
export function movePhoto(
  album: Album,
  from: number,
  slot: number,
  to: number,
): Album {
  if (moveBlocker(album, from, slot, to) !== null) return album;
  const src = album.spreads[from];
  const dst = album.spreads[to];
  const grown = growTemplate(dst.template, dst.slots.length)!;

  const spreads = album.spreads.slice();
  spreads[to] = touched({
    ...dst,
    template: grown.template,
    slots: [...dst.slots, src.slots[slot]],
  });
  const rest = src.slots.filter((_, i) => i !== slot);
  const fb = fallbackTemplate(src.template, rest.length);
  if (!fb) spreads.splice(from, 1);
  else spreads[from] = touched({ ...src, template: fb.template, slots: rest });
  return { ...album, spreads };
}

export type TriEntry = Discard & { manual?: boolean };

/**
 * What the sorting view lists right now: curation entries plus hand-removed
 * photos (in the thumbnail index but neither shown nor listed), minus
 * everything the album currently shows. Rescuing or undoing updates the
 * list with no bookkeeping.
 */
export function triEntries(
  album: Album,
  curation: Discard[],
  thumbSrcs: string[],
): TriEntry[] {
  const shown = new Set(
    album.spreads.flatMap((s) => s.slots.map((sl) => sl.src)),
  );
  const listed = new Set(curation.map((d) => d.src));
  const out: TriEntry[] = curation.filter((d) => !shown.has(d.src));
  for (const src of thumbSrcs) {
    if (!shown.has(src) && !listed.has(src)) {
      out.push({ src, reason: "retiree", focal: [0.5, 0.42], manual: true });
    }
  }
  return out;
}

/** The spread currently showing a photo, or -1. */
export function spreadOf(album: Album, src: string): number {
  return album.spreads.findIndex((s) => s.slots.some((sl) => sl.src === src));
}

/**
 * Bring a discarded photo back into the album. Tried around the anchor
 * spread (the one holding the photo that won over it, else wherever the
 * reader stands): first spread of the three that can grow takes it.
 * Returns the new album and where it landed, or null when all three are full.
 */
export function rescuePhoto(
  album: Album,
  slot: Slot,
  anchor: number,
): { album: Album; at: number } | null {
  const candidates = [anchor, anchor + 1, anchor - 1];
  for (const at of candidates) {
    const spread = album.spreads[at];
    if (!spread) continue;
    const grown = growTemplate(spread.template, spread.slots.length);
    if (!grown) continue;
    const spreads = album.spreads.slice();
    spreads[at] = touched({
      ...spread,
      template: grown.template,
      slots: [...spread.slots, slot],
    });
    return { album: { ...album, spreads }, at };
  }
  return null;
}

/** Swap two photos of the same spread, focal points travelling with them. */
export function swapPhotos(album: Album, at: number, a: number, b: number): Album {
  const spread = album.spreads[at];
  if (!spread || a === b || !spread.slots[a] || !spread.slots[b]) return album;
  const slots = spread.slots.slice();
  [slots[a], slots[b]] = [slots[b], slots[a]];
  return withSpread(album, at, touched({ ...spread, slots }));
}

/** Set a slot's manual crop: focal point and zoom, both clamped. */
export function setSlotCrop(
  album: Album,
  at: number,
  slot: number,
  focal: [number, number],
  zoom: number,
): Album {
  const spread = album.spreads[at];
  const s = spread?.slots[slot];
  if (!s) return album;
  const clamp = (v: number, lo: number, hi: number) =>
    Math.min(Math.max(v, lo), hi);
  const z = clamp(zoom, ZOOM_MIN, ZOOM_MAX);
  const next: Slot = {
    ...s,
    focal: [clamp(focal[0], 0, 1), clamp(focal[1], 0, 1)],
    // The exact fill stays off the file, like serde's default skip.
    ...(z > 1 ? { zoom: z } : {}),
  };
  if (z === 1) delete next.zoom;
  if (
    next.focal[0] === s.focal[0] &&
    next.focal[1] === s.focal[1] &&
    (next.zoom ?? 1) === (s.zoom ?? 1)
  ) {
    return album;
  }
  const slots = spread.slots.slice();
  slots[slot] = next;
  return withSpread(album, at, touched({ ...spread, slots }));
}

/** Set or clear a photo's caption. */
export function setSlotCaption(
  album: Album,
  at: number,
  slot: number,
  caption: string,
): Album {
  const spread = album.spreads[at];
  const s = spread?.slots[slot];
  if (!s) return album;
  const trimmed = caption.trim();
  if ((s.caption ?? "") === trimmed) return album;
  const slots = spread.slots.slice();
  slots[slot] = { ...s, caption: trimmed || undefined };
  return withSpread(album, at, touched({ ...spread, slots }));
}

/** Rename (or clear) a spread's chapter caption, in place. */
export function setSpreadCaption(album: Album, at: number, caption: string): Album {
  const spread = album.spreads[at];
  if (!spread) return album;
  const trimmed = caption.trim();
  if ((spread.caption ?? "") === trimmed) return album;
  return withSpread(
    album,
    at,
    touched({ ...spread, caption: trimmed || undefined }),
  );
}

/** Set the free text of a `texte` spread. */
export function setSpreadText(album: Album, at: number, text: string): Album {
  const spread = album.spreads[at];
  if (!spread) return album;
  if ((spread.text ?? "") === text) return album;
  return withSpread(album, at, touched({ ...spread, text }));
}

/** Toggle the padlock. Locking is not an edit: the badge stays honest. */
export function toggleLock(album: Album, at: number): Album {
  const spread = album.spreads[at];
  if (!spread) return album;
  return withSpread(album, at, { ...spread, locked: !spread.locked });
}

/**
 * Put a drawer photo into a case. The photo already there leaves the album
 * and reappears in the drawer: nothing is ever lost, only displaced.
 */
export function placePhoto(
  album: Album,
  at: number,
  slot: number,
  photo: Slot,
): Album {
  const spread = album.spreads[at];
  if (!spread || !spread.slots[slot]) return album;
  if (spread.slots.some((s, i) => i !== slot && s.src === photo.src)) {
    return album; // already on this spread: a duplicate pair is a defect
  }
  const slots = spread.slots.slice();
  slots[slot] = photo;
  return withSpread(album, at, touched({ ...spread, slots }));
}

/** Move a whole spread to another position. */
export function moveSpread(album: Album, from: number, to: number): Album {
  if (
    from === to ||
    !album.spreads[from] ||
    to < 0 ||
    to >= album.spreads.length
  ) {
    return album;
  }
  const spreads = album.spreads.slice();
  const [spread] = spreads.splice(from, 1);
  spreads.splice(to, 0, touched(spread));
  return { ...album, spreads };
}

/** Duplicate a spread, right after itself. */
export function duplicateSpread(album: Album, at: number): Album {
  const spread = album.spreads[at];
  if (!spread) return album;
  const spreads = album.spreads.slice();
  spreads.splice(at + 1, 0, touched({ ...spread, locked: false }));
  return { ...album, spreads };
}

/** Insert a photo-less spread (breathing page or free text) after `at`. */
export function insertSpread(
  album: Album,
  at: number,
  kind: "vide" | "texte",
): Album {
  const spreads = album.spreads.slice();
  const spread: Spread = {
    template: kind,
    slots: [],
    edited: true,
    ...(kind === "texte" ? { text: "" } : {}),
  };
  spreads.splice(at + 1, 0, spread);
  return { ...album, spreads };
}

/** Remove a whole spread. Undo brings it back, photos and all. */
export function removeSpread(album: Album, at: number): Album {
  if (!album.spreads[at]) return album;
  return withSpread(album, at, null);
}

/** Replace the album's cover. */
export function setCover(album: Album, cover: Cover): Album {
  return { ...album, cover };
}

/**
 * Rename the album. The cover follows when it was never given a title of its
 * own: an untitled cover prints the album's name, and letting the two drift
 * apart at the first rename would be the surprise, not the convenience.
 */
export function renameAlbum(album: Album, title: string): Album {
  const t = title.trim();
  if (t === "" || t === album.title) return album;
  const suivait = !album.cover || album.cover.title.trim() === album.title;
  return {
    ...album,
    title: t,
    cover: album.cover && suivait ? { ...album.cover, title: t } : album.cover,
  };
}

/**
 * Add or drop the colophon page. It lives at the end of the book and nowhere
 * else: a page that says what the object is says it last. Passing null takes
 * it away, which is the one click the Envoi screen offers.
 */
export function setColophon(album: Album, spread: Spread | null): Album {
  const sans = album.spreads.filter((s) => s.template !== COLOPHON_TEMPLATE);
  const spreads = spread ? [...sans, spread] : sans;
  if (spreads.length === album.spreads.length && !spread) return album;
  return { ...album, spreads };
}

/** The album prints its colophon page. */
export function hasColophon(album: Album): boolean {
  return album.spreads.some((s) => s.template === COLOPHON_TEMPLATE);
}

/**
 * Give a spread back the version the composer proposed, badge and lock
 * included. The lock had a way in and no way out; this is the way out.
 * The origin spread comes from `album.origin.json` through the bridge, which
 * is the only file that still holds it: everything else has been edited.
 */
export function restoreSpread(album: Album, at: number, origin: Spread): Album {
  if (!album.spreads[at]) return album;
  return withSpread(album, at, { ...origin, edited: false, locked: false });
}

/** Templates the spread can switch to right now, current one included.
 *  Photo-less templates (capacity 0) never enter the picker. */
export function templateChoices(spread: Spread): [string, number][] {
  return TEMPLATE_CHOICES.filter(
    ([t, cap]) =>
      (cap > 0 && cap <= spread.slots.length) || t === spread.template,
  );
}

// The picker lists plain families and their versos, largest first.
const TEMPLATE_CHOICES: [string, number][] = [...TEMPLATES].sort(
  (x, y) => y[1] - x[1] || x[0].localeCompare(y[0]),
);
