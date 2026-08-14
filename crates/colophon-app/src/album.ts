// Album types and slot geometry. The rectangles below are a port of
// `colophon-core::pdf::slots_for`: same constants, same formulas, so the book
// view and the PDF agree. The PDF stays the reference; this is the working
// preview until pdf.js renders the real thing.

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

export type Album = {
  version: number;
  title: string;
  root: string;
  trim_mm: { w: number; h: number };
  bleed_mm: number;
  spreads: Spread[];
  cover?: Cover;
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

/** Every template with its photo count. Port of `pdf.rs::TEMPLATES`. */
export const TEMPLATES: [string, number][] = [
  ["full1", 1],
  ["full1_verso", 1],
  ["solo", 1],
  ["solo_verso", 1],
  ["solo_paysage", 1],
  ["solo_paysage_verso", 1],
  ["solo_pano", 1],
  ["solo_pano_verso", 1],
  ["solo_etroit", 1],
  ["solo_etroit_verso", 1],
  ["solo_carre", 1],
  ["solo_carre_verso", 1],
  ["duo", 2],
  ["duo_portrait", 2],
  ["duo_paysage", 2],
  ["duo_etroit", 2],
  ["duo_pano", 2],
  ["trio", 3],
  ["trio_verso", 3],
  ["trio_portrait", 3],
  ["trio_portrait_verso", 3],
  ["quad", 4],
  ["quad_portrait", 4],
  ["quad_etroit", 4],
  ["quad_pano", 4],
  ["six", 6],
  ["six_verso", 6],
  ["octo", 8],
  // Photo-less spreads the editor inserts; zero capacity keeps them out of
  // the template picker and of every count-driven rule.
  ["vide", 0],
  ["texte", 0],
];

/** Cell aspects, port of `pdf.rs::CELL_*`. */
const CELL_LANDSCAPE = 4 / 3;
const CELL_PORTRAIT = 0.75;
const CELL_PANO = 2;
const CELL_ETROIT = 0.5;
const CELL_CARRE = 1;

export function templateCapacity(name: string): number {
  return TEMPLATES.find(([t]) => t === name)?.[1] ?? 1;
}

/**
 * Port of `pdf.rs::template_for_count`. Counts without an exact template
 * (5, 7) drop to the largest one below: a grid with a hole in it is worse
 * than one photo fewer.
 */
export function templateForCount(n: number): [string, number] | null {
  if (n <= 0) return null;
  if (n === 1) return ["solo", 1];
  if (n === 2) return ["duo", 2];
  if (n === 3) return ["trio", 3];
  if (n <= 5) return ["quad", 4];
  if (n <= 7) return ["six", 6];
  return ["octo", 8];
}

/** Port of `pdf.rs::fallback_template`: where a spread lands after a loss. */
export function fallbackTemplate(
  current: string,
  remaining: number,
): { template: string; capacity: number } | null {
  const fam = templateForCount(remaining);
  if (!fam) return null;
  const [family, capacity] = fam;
  const verso = `${family}_verso`;
  const keepVerso =
    current.endsWith("_verso") && TEMPLATES.some(([t]) => t === verso);
  return { template: keepVerso ? verso : family, capacity };
}

/** Millimetres, origin top-left of the media canvas (bleed included). */
export type Rect = { x: number; y: number; w: number; h: number };

export type Canvas = {
  w: number;
  h: number;
  margin: number;
  gutter: number;
  /** Bleed on every side; the trimmed spread is the media inset by this much. */
  bleed: number;
};

/** Full media canvas of a spread: two trimmed pages plus bleed all round. */
export function mediaCanvas(album: Album): Canvas {
  const margin = Math.min(album.trim_mm.w, album.trim_mm.h) * (14 / 210);
  return {
    w: album.trim_mm.w * 2 + album.bleed_mm * 2,
    h: album.trim_mm.h + album.bleed_mm * 2,
    margin,
    gutter: margin / 2,
    bleed: album.bleed_mm,
  };
}

/**
 * Share of the margin kept between a chapter caption and the trimmed edge.
 * Port of `pdf.rs::CAPTION_SAFE`.
 */
export const CAPTION_SAFE = 0.5;

/** Slot rectangles for a template, top-left origin, ready for CSS. */
export function slotsFor(template: string, n: number, g: Canvas): Rect[] {
  return slotsBottomUp(template, n, g).map((r) => ({
    x: r.x,
    y: g.h - (r.y + r.h),
    w: r.w,
    h: r.h,
  }));
}

/** The whole of one page, bleed included. Nothing ever spans both. */
function fullPage(right: boolean, g: Canvas): Rect {
  const half = g.w / 2;
  return { x: right ? half : 0, y: 0, w: half, h: g.h };
}

/** Margined content box of one page, half a gutter kept off the fold. */
function pageBox(right: boolean, g: Canvas): Rect {
  const half = g.w / 2;
  const w = half - g.margin - g.gutter / 2;
  return {
    x: right ? half + g.gutter / 2 : g.margin,
    y: g.margin,
    w,
    h: g.h - 2 * g.margin,
  };
}

/** Grid cells inside a box, reading order: top row first, left to right. */
function grid(b: Rect, cols: number, rows: number, gap: number): Rect[] {
  const cw = (b.w - (cols - 1) * gap) / cols;
  const ch = (b.h - (rows - 1) * gap) / rows;
  const out: Rect[] = [];
  for (let r = 0; r < rows; r++) {
    // y grows upward here, so the first row sits at the top
    const y = b.y + (rows - 1 - r) * (ch + gap);
    for (let c = 0; c < cols; c++) {
      out.push({ x: b.x + c * (cw + gap), y, w: cw, h: ch });
    }
  }
  return out;
}

/** A cell of the given aspect ratio, centered in a box. */
function fitted(b: Rect, aspect: number): Rect {
  const w = Math.min(b.w, b.h * aspect);
  const h = w / aspect;
  return { x: b.x + (b.w - w) / 2, y: b.y + (b.h - h) / 2, w, h };
}

/** The engine's own geometry, origin bottom-left as in the PDF. */
function slotsBottomUp(template: string, n: number, g: Canvas): Rect[] {
  // Photo-less spreads hold no rectangles at all.
  if (template === "vide" || template === "texte") return [];
  const verso = template.endsWith("_verso");
  const leadRight = !verso;
  const lead = pageBox(leadRight, g);
  const facing = pageBox(!leadRight, g);
  const base = verso ? template.slice(0, -"_verso".length) : template;

  let v: Rect[];
  switch (base) {
    case "full1":
      v = [fullPage(leadRight, g)];
      break;
    case "solo":
      v = [fitted(lead, CELL_PORTRAIT)];
      break;
    case "solo_paysage":
      v = [fitted(lead, CELL_LANDSCAPE)];
      break;
    case "solo_pano":
      v = [fitted(lead, CELL_PANO)];
      break;
    case "solo_etroit":
      v = [fitted(lead, CELL_ETROIT)];
      break;
    case "solo_carre":
      v = [fitted(lead, CELL_CARRE)];
      break;
    case "duo":
      v = [pageBox(false, g), pageBox(true, g)];
      break;
    case "duo_portrait":
      v = [
        fitted(pageBox(false, g), CELL_PORTRAIT),
        fitted(pageBox(true, g), CELL_PORTRAIT),
      ];
      break;
    case "duo_paysage":
      v = [
        fitted(pageBox(false, g), CELL_LANDSCAPE),
        fitted(pageBox(true, g), CELL_LANDSCAPE),
      ];
      break;
    case "duo_etroit":
      v = [
        fitted(pageBox(false, g), CELL_ETROIT),
        fitted(pageBox(true, g), CELL_ETROIT),
      ];
      break;
    case "duo_pano":
      v = [
        fitted(pageBox(false, g), CELL_PANO),
        fitted(pageBox(true, g), CELL_PANO),
      ];
      break;
    case "trio": {
      const stack = grid(facing, 1, 2, g.gutter).map((c) =>
        fitted(c, CELL_LANDSCAPE),
      );
      v = leadRight
        ? [...stack, fullPage(true, g)]
        : [fullPage(false, g), ...stack];
      break;
    }
    case "trio_portrait": {
      const pair = grid(facing, 2, 1, g.gutter).map((c) =>
        fitted(c, CELL_PORTRAIT),
      );
      v = leadRight
        ? [...pair, fullPage(true, g)]
        : [fullPage(false, g), ...pair];
      break;
    }
    case "quad": {
      const lc = grid(pageBox(false, g), 1, 2, g.gutter);
      const rc = grid(pageBox(true, g), 1, 2, g.gutter);
      v = [lc[0], rc[0], lc[1], rc[1]].map((c) => fitted(c, CELL_LANDSCAPE));
      break;
    }
    case "quad_portrait":
      v = [
        ...grid(pageBox(false, g), 2, 1, g.gutter),
        ...grid(pageBox(true, g), 2, 1, g.gutter),
      ].map((c) => fitted(c, CELL_PORTRAIT));
      break;
    case "quad_etroit":
      v = [
        ...grid(pageBox(false, g), 2, 1, g.gutter),
        ...grid(pageBox(true, g), 2, 1, g.gutter),
      ].map((c) => fitted(c, CELL_ETROIT));
      break;
    case "quad_pano": {
      const lc = grid(pageBox(false, g), 1, 2, g.gutter);
      const rc = grid(pageBox(true, g), 1, 2, g.gutter);
      v = [lc[0], rc[0], lc[1], rc[1]].map((c) => fitted(c, CELL_PANO));
      break;
    }
    case "six": {
      const stack = grid(lead, 1, 2, g.gutter).map((c) =>
        fitted(c, CELL_LANDSCAPE),
      );
      const mosaic = grid(facing, 2, 2, g.gutter);
      v = leadRight ? [...mosaic, ...stack] : [...stack, ...mosaic];
      break;
    }
    case "octo":
      v = [
        ...grid(pageBox(false, g), 2, 2, g.gutter),
        ...grid(pageBox(true, g), 2, 2, g.gutter),
      ];
      break;
    default:
      v = [pageBox(false, g)];
  }
  return v.slice(0, Math.max(n, 1));
}

/** Caption type size: 9 pt, in millimetres. */
export const CAPTION_SIZE_MM = 9 * 0.352778;

/**
 * Where the chapter caption goes, top-left origin. Port of
 * `pdf.rs::caption_anchor`: the first spot no image covers, tried in reading
 * order, because a caption over a full-bleed photo cannot be read.
 */
export function captionAnchor(template: string, n: number, g: Canvas): Rect {
  const rects = slotsBottomUp(template, n, g);
  const half = g.w / 2;
  // Measured from the trimmed edge, not from the media: see pdf.rs.
  const low = g.bleed + g.margin * CAPTION_SAFE;
  const high = g.h - g.bleed - g.margin * 0.75;
  const left = g.bleed + g.margin * 0.57;
  const right = half + g.gutter / 2;

  const candidates = [
    { x: left, y: low },
    { x: right, y: low },
    { x: left, y: high },
    { x: right, y: high },
  ];
  // The ground the printed line actually covers. Port of `pdf.rs::caption_box`.
  const at =
    candidates.find((c) => {
      const b = {
        x: c.x,
        y: c.y - CAPTION_SIZE_MM * 0.3,
        w: g.margin * 3.5,
        h: CAPTION_SIZE_MM * 1.35,
      };
      return rects.every((r) => !overlaps(r, b));
    }) ?? candidates[0];

  return { x: at.x, y: g.h - at.y, w: 0, h: 0 };
}

function overlaps(a: Rect, b: Rect): boolean {
  return (
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
  );
}

/**
 * The part of an image a cover-crop into `rect` shows, in image pixels:
 * `[x0, y0, vw, vh]`, top-left origin. Port of `pdf.rs::crop_window`; the
 * parity test compares both over the engine's sample dump. The crop editor
 * converts pointer deltas with it, so a drag moves the print's crop, not an
 * approximation of it.
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
  const x0 = clamp((iw - vw) * clamp(focal[0], 0, 1), 0, Math.max(iw - vw, 0));
  const y0 = clamp((ih - vh) * clamp(focal[1], 0, 1), 0, Math.max(ih - vh, 0));
  return [x0, y0, vw, vh];
}

/** Hard bounds of the manual zoom: 1 = exact fill, 4 = enough to isolate a
 *  face without ever printing mush. */
export const ZOOM_MIN = 1;
export const ZOOM_MAX = 4;

/** Photo captions: 7 pt, baseline this far under the slot. Port of pdf.rs. */
export const PHOTO_CAPTION_SIZE_MM = 7 * 0.352778;
export const PHOTO_CAPTION_DROP_MM = 3.4;

/** Free-text pages: 11 pt, fixed leading. Port of pdf.rs. */
export const TEXT_SIZE_MM = 11 * 0.352778;
export const TEXT_LEADING_MM = 6.4;

/** First baseline of a `texte` spread, top-left origin (pdf.rs works
 *  bottom-up; the flip happens here like in slotsFor). */
export function textAnchor(g: Canvas): { x: number; y: number } {
  return { x: g.w / 2 + g.gutter / 2, y: g.h - g.h * 0.62 };
}

/** Reference paper weight of the spine coefficients. Port of printer.rs. */
export const GRAMMAGE_REFERENCE = 150;
export const GRAMMAGE_DEFAUT = 150;

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

/** Below this a spine is a fold, not a surface: no title on it. Port of
 *  `cover.rs::SPINE_TEXT_MIN_MM`. */
export const SPINE_TEXT_MIN_MM = 9;
