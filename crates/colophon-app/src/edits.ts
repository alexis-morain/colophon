// Edit operations on an album. All pure: each returns a fresh Album, which is
// what the undo stack stores. The template fallback rule lives in core
// (`pdf.rs::fallback_template`); album.ts carries its port.

import {
  Album,
  Spread,
  TEMPLATES,
  fallbackTemplate,
  templateCapacity,
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
