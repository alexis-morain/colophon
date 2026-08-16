// The ten discard reasons, named once. Every screen that speaks of a
// discard (sorting view, end-of-build report, drawer, keyboard review)
// reads this table: one wording, one order, no drift between surfaces.

/** Sections in display order, with human labels. */
export const REASONS: [string, string][] = [
  ["retiree", "Retirées à la main"],
  ["rejetee", "Rejetées dans votre logiciel photo"],
  ["hors_budget", "Hors budget : bonnes photos, album plein"],
  ["meme_moment", "Même moment, quasi la même photo"],
  ["doublon", "Doublons de rafale ou de scène"],
  ["jumeau", "Quasi identiques"],
  ["panorama", "Panoramas : trop larges pour une page"],
  ["definition", "Définition trop faible pour ce format"],
  ["parasite", "Parasites : captures, images reçues"],
  // No thumbnail exists for these, the review shows the file name alone.
  ["illisible", "Illisibles : fichiers endommagés ou tronqués"],
];

const BY_KEY = new Map(REASONS);

/** The section label of a reason, or the raw key for an unknown one. */
export function reasonLabel(key: string): string {
  return BY_KEY.get(key) ?? key;
}

/** The same label folded into a sentence (drawer hovers): lowercased head. */
export function reasonPhrase(key: string): string {
  const label = BY_KEY.get(key);
  if (!label) return key;
  return label.charAt(0).toLocaleLowerCase("fr") + label.slice(1);
}
