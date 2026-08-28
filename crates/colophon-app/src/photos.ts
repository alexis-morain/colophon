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
import { appliquer } from "./reglage";
import { cachedThumb, loadThumb, meanLuma } from "./thumbs";

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
): Badges {
  if (!img.naturalWidth) return { ppi: null, dark: false, sansMarge: false };
  const connu = Math.max(img.naturalWidth, img.naturalHeight) < THUMB_SIZE;
  const p = effectivePpi(rect, img.naturalWidth, img.naturalHeight, zoom);
  const luma = meanLuma(src, img);
  const room = slidingRoom(
    { w: rect.w * mm, h: rect.h * mm },
    img.naturalWidth,
    img.naturalHeight,
    zoom,
  );
  return {
    ppi: connu && p < MIN_EFFECTIVE_PPI ? Math.round(p) : null,
    // Deliberately the RAW pixels: a photo brightened by a réglage still
    // wears its « sombre » badge. Making the badge (and the analysis's
    // exposure score) read the adjusted values is 4.4, not this session.
    dark: luma !== undefined && luma < DARK_MEAN_LUMA,
    sansMarge: room.x <= ROOM_EPSILON && room.y <= ROOM_EPSILON,
  };
}
