// Album types and slot geometry. The rectangles below are a port of
// `colophon-core::pdf::slots_for`: same constants, same formulas, so the book
// view and the PDF agree. The PDF stays the reference; this is the working
// preview until pdf.js renders the real thing.

export type Slot = { src: string; focal: [number, number] };
export type Spread = { template: string; slots: Slot[]; caption?: string };

export type Album = {
  version: number;
  title: string;
  root: string;
  trim_mm: { w: number; h: number };
  bleed_mm: number;
  spreads: Spread[];
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
  ["duo", 2],
  ["trio", 3],
  ["trio_verso", 3],
  ["quad", 4],
  ["six", 6],
  ["six_verso", 6],
  ["octo", 8],
];

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

export type Canvas = { w: number; h: number; margin: number; gutter: number };

/** Full media canvas of a spread: two trimmed pages plus bleed all round. */
export function mediaCanvas(album: Album): Canvas {
  const margin = Math.min(album.trim_mm.w, album.trim_mm.h) * (14 / 210);
  return {
    w: album.trim_mm.w * 2 + album.bleed_mm * 2,
    h: album.trim_mm.h + album.bleed_mm * 2,
    margin,
    gutter: margin / 2,
  };
}

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
      v = [fitted(lead, 0.75)];
      break;
    case "solo_paysage":
      v = [fitted(lead, 4 / 3)];
      break;
    case "duo":
      v = [pageBox(false, g), pageBox(true, g)];
      break;
    case "trio": {
      const stack = grid(facing, 1, 2, g.gutter);
      v = leadRight
        ? [...stack, fullPage(true, g)]
        : [fullPage(false, g), ...stack];
      break;
    }
    case "quad": {
      const lc = grid(pageBox(false, g), 1, 2, g.gutter);
      const rc = grid(pageBox(true, g), 1, 2, g.gutter);
      v = [lc[0], rc[0], lc[1], rc[1]];
      break;
    }
    case "six": {
      const stack = grid(lead, 1, 2, g.gutter);
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
  const low = g.margin * 0.36;
  const high = g.h - g.margin * 0.75;
  const left = g.margin * 0.57;
  const right = half + g.gutter / 2;

  const candidates = [
    { x: left, y: low },
    { x: right, y: low },
    { x: left, y: high },
    { x: right, y: high },
  ];
  const at =
    candidates.find((c) => {
      const b = {
        x: c.x,
        y: c.y - g.margin * 0.15,
        w: g.margin * 3.5,
        h: g.margin * 0.6,
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
