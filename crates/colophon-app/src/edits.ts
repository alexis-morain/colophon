// Edit operations on an album. All pure: each returns a fresh Album, which is
// what the undo stack stores. The template fallback rule lives in core
// (`pdf.rs::fallback_template`); album.ts carries its port.

import {
  Album,
  Discard,
  Slot,
  Spread,
  TEMPLATES,
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
  return withSpread(album, at, {
    ...spread,
    template: fb.template,
    slots: slots.slice(0, fb.capacity),
  });
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
  return withSpread(album, at, {
    ...spread,
    template,
    slots: spread.slots.slice(0, cap),
  });
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
  spreads[to] = {
    ...dst,
    template: grown.template,
    slots: [...dst.slots, src.slots[slot]],
  };
  const rest = src.slots.filter((_, i) => i !== slot);
  const fb = fallbackTemplate(src.template, rest.length);
  if (!fb) spreads.splice(from, 1);
  else spreads[from] = { ...src, template: fb.template, slots: rest };
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
    spreads[at] = {
      ...spread,
      template: grown.template,
      slots: [...spread.slots, slot],
    };
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
  return withSpread(album, at, { ...spread, slots });
}

/** Templates the spread can switch to right now, current one included. */
export function templateChoices(spread: Spread): [string, number][] {
  return TEMPLATE_CHOICES.filter(
    ([t, cap]) => cap <= spread.slots.length || t === spread.template,
  );
}

// The picker lists plain families and their versos, largest first.
const TEMPLATE_CHOICES: [string, number][] = [...TEMPLATES].sort(
  (x, y) => y[1] - x[1] || x[0].localeCompare(y[0]),
);
