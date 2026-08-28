// Thumbnail loading. The backend hands over raw JPEG bytes; we keep a bounded
// pool of blob URLs so flipping through a 50-spread album never holds more
// than a few dozen images in memory.

import type { Reglage } from "./album";
import { fetchThumb } from "./bridge";
import { transfert } from "./reglage";

// Sized for the sorting view: a couple of screens of grid cells plus the
// book view's neighbourhood, without ever holding a whole album.
const LIMIT = 240;
const urls = new Map<string, string>();
const pending = new Map<string, Promise<string>>();

export function cachedThumb(src: string): string | undefined {
  const url = urls.get(src);
  if (url) {
    // touch: re-insert to move to the end of the eviction order
    urls.delete(src);
    urls.set(src, url);
  }
  return url;
}

export function loadThumb(src: string): Promise<string> {
  const hit = cachedThumb(src);
  if (hit) return Promise.resolve(hit);

  const inflight = pending.get(src);
  if (inflight) return inflight;

  const job = fetchThumb(src)
    .then((bytes) => {
      const url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
      urls.set(src, url);
      evict();
      return url;
    })
    .finally(() => {
      pending.delete(src);
    });

  pending.set(src, job);
  return job;
}

function evict() {
  while (urls.size > LIMIT) {
    const oldest = urls.keys().next();
    if (oldest.done) return;
    URL.revokeObjectURL(urls.get(oldest.value)!);
    urls.delete(oldest.value);
  }
}

/** Drop everything: called when another album is opened. */
export function resetThumbs() {
  for (const url of urls.values()) URL.revokeObjectURL(url);
  urls.clear();
  pending.clear();
  lumas.clear();
}

/**
 * What one thumbnail's luminance is worth, sampled once — two quantities,
 * because they answer two different questions and neither derives from the
 * other without a loss.
 *
 * `moyenne` is the exact mean of the unrounded luma. `DARK_MEAN_LUMA` was
 * measured against *that* number, so it is kept as it always was: a
 * threshold must not move because a histogram rounds.
 *
 * `histo` is the same samples in 256 bins. A réglage needs the distribution
 * and not its mean, because the mean of a corrected photograph is not the
 * correction of its mean — see `moyenneCorrigee`. A kilobyte per photo,
 * against a blob URL pool that holds whole images.
 */
type Luma = { moyenne: number; histo: Uint32Array };

// Keyed by src and never evicted. Raw pixels only: no corrected value ever
// enters this map, so « Rendre à l'original » gives the original badge back
// without waiting for a reload.
const lumas = new Map<string, Luma>();

/**
 * Sample a decoded thumbnail's luminance, once per photo. 0..255, mixed in
 * 601, the same quantity `analyze.rs::exposure_score` averages. Sampled on a
 * 64 px square: the engine works at 128, and the extra precision buys
 * nothing for a threshold no photo of the reference sets sits near.
 */
function lumaDe(src: string, img: HTMLImageElement): Luma | undefined {
  const seen = lumas.get(src);
  if (seen) return seen;
  if (!img.complete || !img.naturalWidth) return undefined;
  if (typeof document === "undefined") return undefined;
  const size = 64;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return undefined;
  ctx.drawImage(img, 0, 0, size, size);
  let sum = 0;
  const histo = new Uint32Array(256);
  try {
    const { data } = ctx.getImageData(0, 0, size, size);
    for (let i = 0; i < data.length; i += 4) {
      const y = 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
      sum += y;
      histo[Math.round(y)] += 1;
    }
  } catch {
    return undefined; // a tainted canvas is not worth an exception
  }
  const out = { moyenne: sum / (size * size), histo };
  lumas.set(src, out);
  return out;
}

/** Mean luminance of a decoded thumbnail, 0..255: what the photograph is,
 *  before any adjustment. */
export function meanLuma(src: string, img: HTMLImageElement): number | undefined {
  return lumaDe(src, img)?.moyenne;
}

/** The same sampling, kept as a distribution: what an adjustment needs. */
export function histoLuma(
  src: string,
  img: HTMLImageElement,
): Uint32Array | undefined {
  return lumaDe(src, img)?.histo;
}

/**
 * The mean luminance a photograph will print at, once its réglage is burnt
 * in: the transfer applied bin by bin, then averaged. Never the transfer of
 * the mean.
 *
 * The two are not the same because the transfer clamps. Between its clamps
 * it is affine — exposure scales, contrast pivots around 0,5 — and an affine
 * function does commute with an average; it is the clipping at each end that
 * breaks it. Which is exactly the case that matters here: lifting a night
 * shot crushes its highlights against 1, and the mean of the clipped
 * photograph is lower than the mean the affine part predicts. Averaging
 * first would report a photograph nobody will print.
 *
 * The transfer is mono-channel and runs on the sampled 601 luma, while `nb`
 * greys in 709. For a colour photograph that is an approximation — a LUT per
 * channel then mixed is not the transfer of the mix — exact on greys,
 * monotonic everywhere. The badge is a threshold, not a measurement, and
 * `DARK_MEAN_LUMA` was measured under these coefficients: harmonising them
 * would move a measured threshold, not fix anything. `nb` itself is
 * therefore not in the formula, which is first-order right — greying
 * preserves the luminance it is a mix of.
 */
export function moyenneCorrigee(histo: Uint32Array, r: Reglage): number {
  let somme = 0;
  let n = 0;
  for (let k = 0; k < 256; k++) {
    const cases = histo[k];
    if (cases === 0) continue;
    n += cases;
    somme += cases * transfert(k / 255, r.expo, r.contraste);
  }
  return n === 0 ? 0 : (somme / n) * 255;
}
