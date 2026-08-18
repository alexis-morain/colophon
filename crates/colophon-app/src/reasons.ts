// The ten discard reasons, named once. Every screen that speaks of a
// discard (sorting view, end-of-build report, drawer, keyboard review)
// reads this table: one wording, one order, no drift between surfaces.
// The wording itself lives in the dictionaries (`raison.*`).

import { Cle, langue, t } from "./i18n";

/** Sections in display order. */
export const REASON_KEYS = [
  "retiree",
  "rejetee",
  "hors_budget",
  "meme_moment",
  "doublon",
  "jumeau",
  "panorama",
  "definition",
  "parasite",
  // No thumbnail exists for these, the review shows the file name alone.
  "illisible",
] as const;

/** The section label of a reason, or the raw key for an unknown one. */
export function reasonLabel(key: string): string {
  return (REASON_KEYS as readonly string[]).includes(key)
    ? t(`raison.${key}` as Cle)
    : key;
}

/** The same label folded into a sentence (drawer hovers): lowercased head. */
export function reasonPhrase(key: string): string {
  if (!(REASON_KEYS as readonly string[]).includes(key)) return key;
  const label = reasonLabel(key);
  return label.charAt(0).toLocaleLowerCase(langue()) + label.slice(1);
}
