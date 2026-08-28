// The thumbnails a spread needs, as decoded images, and the two things the
// editor says about a photograph once it has one.
//
// `thumbs.ts` caches the *URL* of a thumbnail, which is all an `<img>` tag
// needs. A canvas needs the decoded image itself, and so does anyone who
// wants to know how many pixels a photograph has without reopening the
// original. Hence one cache, shared: the DOM renderer reads its badges from
// the element it already drew, the canvas reads them from here, and the rule
// they read them by is written once.

import {
  DARK_MEAN_LUMA,
  MIN_EFFECTIVE_PPI,
  Rect,
  Reglage,
  THUMB_SIZE,
  effectivePpi,
  slidingRoom,
} from "./album";
import { appliquer, estIdentite } from "./reglage";
import {
  cachedThumb,
  histoLuma,
  loadThumb,
  meanLuma,
  moyenneCorrigee,
} from "./thumbs";

const images = new Map<string, HTMLImageElement>();
const enCours = new Set<string>();
const abonnes = new Set<(src: string) => void>();

/**
 * The decoded thumbnail of a source, or null while it is on its way. Asking
 * starts the loading; whoever is subscribed hears about it when it lands.
 */
export function imageDe(src: string): HTMLImageElement | null {
  const prete = images.get(src);
  if (prete) return prete;
  if (enCours.has(src) || typeof Image === "undefined") return null;
  enCours.add(src);
  const poser = (url: string) => {
    const img = new Image();
    img.onload = () => {
      enCours.delete(src);
      images.set(src, img);
      abonnes.forEach((f) => f(src));
    };
    img.onerror = () => enCours.delete(src);
    img.src = url;
  };
  const dejaLa = cachedThumb(src);
  if (dejaLa) poser(dejaLa);
  else loadThumb(src).then(poser, () => enCours.delete(src));
  return null;
}

/** One pre-adjusted bitmap per photo — the last committed réglage only, so
 *  the pool stays the size of the thumbnail pool. */
const reglees = new Map<string, { cle: string; bitmap: HTMLCanvasElement }>();

/**
 * The thumbnail of a photo with its committed adjustment burnt in: the
 * canvas renderer's fallback where `ctx.filter` does not exist. Computed
 * lazily at the repaint a commit triggers, cached by (src, réglage), and
 * never during a slider drag — the caller ignores the draft on this path,
 * so the case follows at release, and that is all. The thumbnail cache on
 * disk stays untouched: this adjusts a copy, in memory, for the screen.
 */
export function imageRegleeDe(src: string, r: Reglage): CanvasImageSource | null {
  const img = imageDe(src);
  if (!img) return null;
  const cle = `${r.expo}|${r.contraste}|${r.nb}`;
  const hit = reglees.get(src);
  if (hit && hit.cle === cle) return hit.bitmap;
  const c = document.createElement("canvas");
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  const ctx = c.getContext("2d");
  if (!ctx) return img;
  ctx.drawImage(img, 0, 0);
  const data = ctx.getImageData(0, 0, c.width, c.height);
  appliquer(data.data, r);
  ctx.putImageData(data, 0, 0);
  reglees.set(src, { cle, bitmap: c });
  return c;
}

/** Runs whenever a thumbnail finishes decoding. */
export function surImage(hook: (src: string) => void): () => void {
  abonnes.add(hook);
  return () => {
    abonnes.delete(hook);
  };
}

/** What a case says about itself, over the photograph. */
export type Badges = {
  /** Effective resolution, only when it is both known and under the floor. */
  ppi: number | null;
  dark: boolean;
  /** The photograph fills its cell exactly: no gesture can slide it. */
  sansMarge: boolean;
};

/** Under half a pixel each way, no gesture can move anything. Read by the
 *  badge below and by the two crop gestures, so it is declared once. */
export const ROOM_EPSILON = 0.5;

/**
 * Whether the photograph prints dark — the photograph as it will print,
 * réglage included. A badge that judged the original would tell someone who
 * has just rescued a night shot that it is still too dark, which is the
 * badge lying about the book it claims to describe.
 *
 * Without an adjustment, and under one that adjusts nothing, this is the raw
 * mean to the bit: same cache, same value, same photos marked. The corrected
 * mean is computed at the read, from the histogram, and never cached beside
 * the raw one — so « Rendre à l'original » gives the original badge back at
 * once.
 *
 * The réglage handed in is the album's, never the draft: the caller passes
 * `reglagePose`, so a moving slider does not make the badge blink about a
 * réglage the album has not accepted yet.
 */
function estSombre(
  src: string,
  img: HTMLImageElement,
  reglage?: Reglage,
): boolean {
  if (!reglage || estIdentite(reglage)) {
    const brut = meanLuma(src, img);
    return brut !== undefined && brut < DARK_MEAN_LUMA;
  }
  const histo = histoLuma(src, img);
  return histo !== undefined && moyenneCorrigee(histo, reglage) < DARK_MEAN_LUMA;
}

/**
 * The two warnings and the one fact, from the thumbnail already on screen —
 * no engine round-trip.
 *
 * Resolution is only asserted when it is known: a thumbnail under
 * `THUMB_SIZE` was never downscaled, so its pixel count is the original's. A
 * downscaled one proves the original is bigger, so a computed ppi *above*
 * the floor clears the photograph, while one below it proves nothing and no
 * badge shows. The preflight, which reopens the originals, stays the
 * authority at export time.
 */
export function badgesDe(
  src: string,
  img: HTMLImageElement,
  rect: Rect,
  mm: number,
  zoom: number,
  reglage?: Reglage,
): Badges {
  if (!img.naturalWidth) return { ppi: null, dark: false, sansMarge: false };
  const connu = Math.max(img.naturalWidth, img.naturalHeight) < THUMB_SIZE;
  const p = effectivePpi(rect, img.naturalWidth, img.naturalHeight, zoom);
  const room = slidingRoom(
    { w: rect.w * mm, h: rect.h * mm },
    img.naturalWidth,
    img.naturalHeight,
    zoom,
  );
  return {
    ppi: connu && p < MIN_EFFECTIVE_PPI ? Math.round(p) : null,
    // Resolution stays a fact about the original — a réglage moves no pixel
    // count — while darkness is a fact about the print, and reads through
    // the adjustment. The analysis's own exposure score does not: it is a
    // scalar of the original, and nothing can unfold it. See
    // `analyze.rs::exposure_score`, which says so where it is computed.
    dark: estSombre(src, img, reglage),
    sansMarge: room.x <= ROOM_EPSILON && room.y <= ROOM_EPSILON,
  };
}
