// Album types and slot geometry. Since the templates became data
// (`gabarit::catalogue`), the editor draws every rectangle, anchor and type
// constant from the engine's geometry dump (`geometrie.ts`), loaded before
// the album renders: no dimension is declared twice any more. What remains
// written here is the algorithmic residue the dump cannot carry (crop
// windows, cover sheet, half-title layout), each pinned by the parity test.

import { geometrie, geometrieCourante, onGeometrie } from "./geometrie";

export type Slot = {
  src: string;
  focal: [number, number];
  /** Manual zoom past the cover fill, absent = 1. Never below 1. */
  zoom?: number;
  /** Caption printed under the photo. */
  caption?: string;
};
export type Spread = {
  template: string;
  slots: Slot[];
  caption?: string;
  /** Free text of a `texte` spread, lines printed as typed. */
  text?: string;
  /** Touched by hand: survives any recomposition, wears the badge. */
  edited?: boolean;
  /** Pinned without being edited: same recomposition shield. */
  locked?: boolean;
};

export type Cover = {
  title: string;
  subtitle?: string;
  photo?: Slot;
  back_text?: string;
};

/** One photograph's adjustments, mirror of `model.rs::Reglage`. The
 *  transform the numbers name is defined in core (`reglage.rs`) and ported
 *  in `reglage.ts`; it is the CSS filter formula, which is what lets the
 *  screen show it with one line of style. */
export type Reglage = {
  /** Exposure, in stops of `brightness(2^expo)`, clamped to ±1. */
  expo: number;
  /** Contrast, `contrast(2^contraste)` around the 0,5 pivot, clamped to ±1. */
  contraste: number;
  /** Black and white: luma 709, the coefficients of `grayscale(1)`. */
  nb: boolean;
};

export type Album = {
  version: number;
  title: string;
  root: string;
  trim_mm: { w: number; h: number };
  bleed_mm: number;
  spreads: Spread[];
  cover?: Cover;
  /** What the composition measured, kept for the colophon page. Absent on
   *  albums composed before the page existed: the page is then not offered,
   *  because nothing here can be invented after the fact. The shape is
   *  opaque to the front, which only asks the engine to render it. */
  colophon?: unknown;
  /** Non-destructive adjustments, keyed by `Slot::src`: a property of the
   *  photograph, never of the cell. Applied at render time only, never an
   *  octet on the original. Absent = none; an identity entry leaves the
   *  table at the edit that produced it (`edits.ts::setReglage`). */
  reglages?: Record<string, Reglage>;
};

export type OpenedAlbum = {
  album: Album;
  dir: string;
  root_present: boolean;
  /** Every source with a cached thumbnail: shown photos plus discarded. */
  thumb_srcs: string[];
};

/** One photo curation set aside, as written in curation.json. */
export type Discard = {
  src: string;
  reason: string;
  kept?: string;
  focal: [number, number];
};

/** Every template with its photo count, in catalogue order, straight from
 *  the engine's dump. */
export function templates(): [string, number][] {
  return geometrieCourante().ordre;
}

export function templateCapacity(name: string): number {
  return templates().find(([t]) => t === name)?.[1] ?? 1;
}

/** The engine's own count table (`fallbacks` in the dump). Counts without
 *  an exact template (5, 7) drop to the largest one below. */
export function templateForCount(n: number): [string, number] | null {
  if (n <= 0) return null;
  const table = geometrieCourante().fallbacks;
  return table[String(Math.min(n, 9))] ?? null;
}

/** Where a spread lands after a loss, `_verso` side kept when it exists. */
export function fallbackTemplate(
  current: string,
  remaining: number,
): { template: string; capacity: number } | null {
  const fam = templateForCount(remaining);
  if (!fam) return null;
  const [family, capacity] = fam;
  const verso = `${family}_verso`;
  const keepVerso =
    current.endsWith("_verso") && templates().some(([t]) => t === verso);
  return { template: keepVerso ? verso : family, capacity };
}

/** Millimetres, origin top-left of the spread's media box (bleed included). */
export type Rect = { x: number; y: number; w: number; h: number };

export type SpreadGeometry = {
  w: number;
  h: number;
  margin: number;
  gutter: number;
  /** Bleed on every side; the trimmed spread is the media inset by this much. */
  bleed: number;
};

/** Full geometry of a spread, from the dump: the media box of two trimmed
 *  pages plus bleed all round, and the margins the engine derived. Mirror of
 *  `pdf::geometry`, whose struct carries the same name. */
export function spreadGeometry(album: Album): SpreadGeometry {
  const d = geometrie(album.trim_mm, album.bleed_mm);
  return { ...d.media, bleed: d.bleed_mm };
}

/**
 * Share of the margin kept between a chapter caption and the trimmed edge.
 * Hydrated from the dump, like every constant below.
 */
export let CAPTION_SAFE = 0.5;

/** The trim behind a geometry: exact, because the media box was built from
 *  it by additions a subtraction undoes without loss. */
function trimOf(g: SpreadGeometry): { w: number; h: number } {
  return { w: (g.w - 2 * g.bleed) / 2, h: g.h - 2 * g.bleed };
}

/** Slot rectangles for a template, top-left origin, ready for CSS: the
 *  engine's own rectangles, truncated like `slots_for` truncates. */
export function slotsFor(template: string, n: number, g: SpreadGeometry): Rect[] {
  const d = geometrie(trimOf(g), g.bleed);
  const t = d.templates[template];
  // A template the catalogue does not know (album.json repaired by hand):
  // the engine falls back to one margined box, and so does the view.
  const slots = t
    ? t.slots
    : [
        [
          g.margin,
          g.margin,
          g.w / 2 - g.margin - g.gutter / 2,
          g.h - 2 * g.margin,
        ],
      ];
  return slots.slice(0, Math.max(n, 1)).map(([x, y, w, h]) => ({
    x,
    y: g.h - (y + h),
    w,
    h,
  }));
}

/** Caption type size: 9 pt, in millimetres (hydrated). */
export let CAPTION_SIZE_MM = 9 * 0.352778;

/**
 * Where the chapter caption goes, top-left origin: the engine computed one
 * anchor per photo count, because the free spot moves with the rectangles.
 */
export function captionAnchor(template: string, n: number, g: SpreadGeometry): Rect {
  const d = geometrie(trimOf(g), g.bleed);
  const t = d.templates[template] ?? d.templates["vide"];
  const caps = t.captions;
  const at = caps[Math.max(0, Math.min(n, caps.length - 1))];
  return { x: at[0], y: g.h - at[1], w: 0, h: 0 };
}

/**
 * The part of an image a cover-crop into `rect` shows, in image pixels:
 * `[x0, y0, vw, vh]`, top-left origin. Port of `pdf.rs::crop_window`; the
 * parity test compares both over the engine's sample dump. The crop editor
 * converts pointer deltas with it, so a drag moves the print's crop, not an
 * approximation of it.
 *
 * `focal` is a point of the image, as a fraction of its width and height:
 * the window centres on it, and only the image borders may move it off
 * centre. Ratio-invariant by construction — see the Rust twin for why the
 * pre-schema-2 meaning was not.
 */
export function cropWindow(
  rect: { w: number; h: number },
  iw: number,
  ih: number,
  focal: [number, number],
  zoom: number,
): [number, number, number, number] {
  const s = Math.max(rect.w / iw, rect.h / ih) * Math.max(zoom, 1);
  const vw = rect.w / s;
  const vh = rect.h / s;
  const clamp = (v: number, lo: number, hi: number) =>
    Math.min(Math.max(v, lo), hi);
  const x0 = clamp(clamp(focal[0], 0, 1) * iw - vw / 2, 0, Math.max(iw - vw, 0));
  const y0 = clamp(clamp(focal[1], 0, 1) * ih - vh / 2, 0, Math.max(ih - vh, 0));
  return [x0, y0, vw, vh];
}

/**
 * How far a photo can slide inside its cell, in the unit `rect` is given in.
 *
 * A cover-crop scales the photo until it covers the cell, so the room to slide
 * is whatever hangs over the edges. When photo and cell share an aspect ratio
 * the overhang is zero on both axes and there is nothing to drag — and that is
 * not a corner case: the composer places photos in cells fitted to their
 * shape, so the better it does its job the more often this happens. Measured
 * on the reference sets, 26 % to 82 % of placed photos have no room at all at
 * zoom 1. Zooming past the fill is what buys it back.
 *
 * Same arithmetic as `cropWindow` above, read the other way round: that one
 * says which pixels show, this one says how many are left over.
 */
export function slidingRoom(
  rect: { w: number; h: number },
  iw: number,
  ih: number,
  zoom = 1,
): { x: number; y: number } {
  if (iw <= 0 || ih <= 0) return { x: 0, y: 0 };
  const { x, y } = imageSpan(rect, iw, ih, zoom);
  return { x: x - rect.w, y: y - rect.h };
}

/**
 * How much room the whole photo takes, in the unit `rect` is given in — the
 * cover-crop scale applied to the full image, overhang included.
 *
 * This is the denominator of a manual crop since schema 2: `focal` is a point
 * of the image, so dragging by one unit moves it by `1 / span` of the image,
 * whatever the cell's ratio. `slidingRoom` above is this minus the cell, and
 * answers the other question — whether a drag can move anything at all.
 */
export function imageSpan(
  rect: { w: number; h: number },
  iw: number,
  ih: number,
  zoom = 1,
): { x: number; y: number } {
  if (iw <= 0 || ih <= 0) return { x: 0, y: 0 };
  const s = Math.max(rect.w / iw, rect.h / ih) * Math.max(zoom, 1);
  return { x: iw * s, y: ih * s };
}

/** Hard bounds of the manual zoom: 1 = exact fill, 4 = enough to isolate a
 *  face without ever printing mush. */
export const ZOOM_MIN = 1;
export const ZOOM_MAX = 4;

/** Photo captions: baseline this far under the slot (hydrated). */
export let PHOTO_CAPTION_SIZE_MM = 7 * 0.352778;
export let PHOTO_CAPTION_DROP_MM = 3.4;

/** Free-text pages (hydrated). */
export let TEXT_SIZE_MM = 11 * 0.352778;
export let TEXT_LEADING_MM = 6.4;

/** First baseline of a `texte` spread, top-left origin (the engine works
 *  bottom-up; the flip happens here like in slotsFor). */
export function textAnchor(g: SpreadGeometry): { x: number; y: number } {
  const [x, y] = geometrie(trimOf(g), g.bleed).anchors.texte;
  return { x, y: g.h - y };
}

/** The colophon spread: quieter than a text page and low on the recto. */
export const COLOPHON_TEMPLATE = "colophon";
export let COLOPHON_SIZE_MM = 8.5 * 0.352778;
export let COLOPHON_LEADING_MM = 4.6;

export function colophonAnchor(g: SpreadGeometry): { x: number; y: number } {
  const [x, y] = geometrie(trimOf(g), g.bleed).anchors.colophon;
  return { x, y: g.h - y };
}

/** The half-title spread: two sizes on one page, so it carries its own
 *  layout (`gardeLayout`), the algorithm pinned by the dump's samples. */
export const GARDE_TEMPLATE = "garde";
export let GARDE_TITRE_MM = 18 * 0.352778;
export let GARDE_TITRE_MIN_MM = 8.5 * 0.352778;
export let GARDE_LIGNE_MM = 9.5 * 0.352778;
export let GARDE_LIGNE_LEADING_MM = 5;
export let GARDE_APRES_TITRE_MM = 14;

/** The longest title the field accepts. The engine measured, on the six
 *  formats and in the face the PDF embeds, that a title of this length
 *  still prints whole on the half-title (hydrated). */
export let TITRE_MAX = 64;

/** The title the book wears: the cover's when it was given one of its own,
 *  the album's name otherwise. Port of `cover.rs::titre_du_livre`. The
 *  half-title prints this, so a book called « Un été » on its cover is not
 *  called something else three pages later. */
export function titreDuLivre(album: Album): string {
  const c = album.cover?.title.trim();
  return c ? c : album.title;
}

export function gardeAnchor(g: SpreadGeometry): { x: number; y: number } {
  const [x, y] = geometrie(trimOf(g), g.bleed).anchors.garde;
  return { x, y: g.h - y };
}

/** The room a line has on that page: the recto's margined box. */
export function gardePlace(g: SpreadGeometry): number {
  return geometrie(trimOf(g), g.bleed).anchors.garde_place;
}

/** One line of the half-title, ready to draw. Port of `garde.rs::Ligne`,
 *  with `dy` growing downward like everything else on this side. */
export type GardeLigne = { texte: string; tailleMm: number; dyMm: number };

/**
 * The stored text laid out: the first line is the title, at the size that
 * fits the page, and every other non-empty line is quiet under it. The blank
 * line of the stored text is spacing, not a line to draw.
 *
 * `mesure` is the caller's own width measurement in the embedded face: the
 * engine fits the title against the widths the PDF draws, and the editor has
 * to fit it against the same ones or show a size the print will not use.
 */
export function gardeLayout(
  text: string,
  placeMm: number,
  mesure: (s: string, tailleMm: number) => number,
): GardeLigne[] {
  const lignes = text.split("\n");
  if (lignes.length === 0) return [];
  const titre = lignes[0];
  const large = mesure(titre, GARDE_TITRE_MM);
  const taille =
    large <= placeMm || large <= 0
      ? GARDE_TITRE_MM
      : Math.max((GARDE_TITRE_MM * placeMm) / large, GARDE_TITRE_MIN_MM);
  const out: GardeLigne[] = [{ texte: titre, tailleMm: taille, dyMm: 0 }];
  lignes
    .slice(1)
    .filter((l) => l.trim() !== "")
    .forEach((l, i) =>
      out.push({
        texte: l,
        tailleMm: GARDE_LIGNE_MM,
        dyMm: GARDE_APRES_TITRE_MM + i * GARDE_LIGNE_LEADING_MM,
      }),
    );
  return out;
}

/** Reference paper weight of the spine coefficients (hydrated). */
export let GRAMMAGE_REFERENCE = 150;
export let GRAMMAGE_DEFAUT = 150;

/** The spine parameters a printer profile carries, as `printer.rs`
 *  serialises them. Declared here so the geometry below takes plain data and
 *  the parity check can feed it without the bridge. */
export type DosProfil =
  | { mode: "fourni" }
  | { mode: "calcule"; mm_par_feuille: number; constante_mm: number };

/**
 * Spine width in millimetres, or null when the supplier builds its own.
 * Port of `PrinterProfile::dos_mm`: the coefficients belong to the profile
 * and never to this file, so the day Cloudprinter confirms its formula one
 * edit moves both the editor and the printed sheet. The provisional value is
 * flagged where it shows, not here.
 */
export function spineMm(
  dos: DosProfil,
  pages: number,
  grammage = GRAMMAGE_DEFAUT,
): number | null {
  if (dos.mode === "fourni") return null;
  // A sheet is two pages, and heavier paper thickens the spine pro rata.
  const feuilles = pages / 2;
  return (
    feuilles * dos.mm_par_feuille * (grammage / GRAMMAGE_REFERENCE) +
    dos.constante_mm
  );
}

/** The flat cover sheet, in millimetres. Port of `cover.rs::geometry`: back
 *  cover, spine, front, plus the profile's bleed on the outer edges. */
export type CoverSheet = {
  w: number;
  h: number;
  /** `[x, width]` of each trim panel, left to right. */
  back: [number, number];
  spine: [number, number] | null;
  front: [number, number];
};

export function coverSheet(
  album: { trim_mm: { w: number; h: number }; spreads: unknown[] },
  profil: {
    dos: DosProfil;
    bleed_mm: { haut: number; bas: number; exterieur: number };
  },
): CoverSheet {
  const pages = album.spreads.length * 2;
  const spine = spineMm(profil.dos, pages);
  const s = spine ?? 0;
  const ext = profil.bleed_mm.exterieur;
  return {
    w: album.trim_mm.w * 2 + s + ext * 2,
    h: album.trim_mm.h + profil.bleed_mm.haut + profil.bleed_mm.bas,
    back: [ext, album.trim_mm.w],
    spine: spine === null ? null : [ext + album.trim_mm.w, spine],
    front: [ext + album.trim_mm.w + s, album.trim_mm.w],
  };
}

/** Below this a spine is a fold, not a surface: no title on it (hydrated). */
export let SPINE_TEXT_MIN_MM = 9;

/** Longest side of a cached thumbnail (hydrated from `thumb.rs`).
 *  A thumbnail below this size was never downscaled, so its pixel count is
 *  the original's: the only case where the editor knows a photo's true
 *  resolution without reopening the file. */
export let THUMB_SIZE = 1600;

/** Below this effective resolution a cell prints visibly soft: the floor
 *  the preflight blocks on (hydrated from `audit.rs`). */
export let MIN_EFFECTIVE_PPI = 250;

/**
 * Effective print resolution of a photo in a cell, in ppi. Port of the
 * preflight's own rule (`prevol.rs`, règle `resolution`): the cover-crop
 * scale of `print.rs::print_scale`, times the manual zoom, which crops
 * further into the same pixels.
 */
export function effectivePpi(
  rect: { w: number; h: number },
  pixelW: number,
  pixelH: number,
  zoom = 1,
): number {
  if (pixelW <= 0 || pixelH <= 0) return Infinity;
  const mmPerPixel = Math.max(rect.w / pixelW, rect.h / pixelH);
  return 25.4 / (mmPerPixel * Math.max(zoom, 1));
}

/** Mean luminance under which a photo prints noticeably darker than it
 *  looks on screen, on the 0..255 scale `analyze.rs::exposure_score` works
 *  in. Measured on the reference sets rather than guessed: at 60 it marks
 *  4 of the 294 placed photos of corse-2013, mauritanie-2019 and
 *  random-2024, all four real night shots. The engine's own dark penalty
 *  starts at 70, which would have marked ordinary evening photos too, and
 *  a badge that fires on ordinary photos teaches people to ignore badges. */
export const DARK_MEAN_LUMA = 60;

// ---- hydration ----------------------------------------------------------
// The bindings above keep their historical defaults only until the first
// dump lands; from then on the engine's numbers are the numbers. `let` plus
// a hook, so the two hundred call sites read a constant and the constant
// still has exactly one source.
onGeometrie((d) => {
  const c = d.constantes;
  CAPTION_SAFE = c.caption_safe;
  CAPTION_SIZE_MM = c.caption_size_mm;
  PHOTO_CAPTION_SIZE_MM = c.photo_caption_size_mm;
  PHOTO_CAPTION_DROP_MM = c.photo_caption_drop_mm;
  TEXT_SIZE_MM = c.text_size_mm;
  TEXT_LEADING_MM = c.text_leading_mm;
  COLOPHON_SIZE_MM = c.colophon_size_mm;
  COLOPHON_LEADING_MM = c.colophon_leading_mm;
  GARDE_TITRE_MM = c.garde_titre_mm;
  GARDE_TITRE_MIN_MM = c.garde_titre_min_mm;
  GARDE_LIGNE_MM = c.garde_ligne_mm;
  GARDE_LIGNE_LEADING_MM = c.garde_ligne_leading_mm;
  GARDE_APRES_TITRE_MM = c.garde_apres_titre_mm;
  TITRE_MAX = c.titre_max;
  SPINE_TEXT_MIN_MM = c.spine_text_min_mm;
  GRAMMAGE_REFERENCE = c.grammage_reference;
  GRAMMAGE_DEFAUT = c.grammage_defaut;
  MIN_EFFECTIVE_PPI = c.min_effective_ppi;
  THUMB_SIZE = c.thumb_size;
});
