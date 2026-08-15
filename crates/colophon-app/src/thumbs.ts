// Thumbnail loading. The backend hands over raw JPEG bytes; we keep a bounded
// pool of blob URLs so flipping through a 50-spread album never holds more
// than a few dozen images in memory.

import { fetchThumb } from "./bridge";

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
  luma.clear();
}

// Mean luminance per photo, computed once from the thumbnail already on
// screen. Keyed by src and never evicted: one number per photo, against a
// blob URL pool that holds whole images.
const luma = new Map<string, number>();

/**
 * Mean luminance of a decoded thumbnail, 0..255, the same quantity
 * `analyze.rs::exposure_score` averages. Sampled on a 64 px square: the
 * engine works at 128, and the extra precision buys nothing for a threshold
 * no photo of the reference sets sits near.
 */
export function meanLuma(src: string, img: HTMLImageElement): number | undefined {
  const seen = luma.get(src);
  if (seen !== undefined) return seen;
  if (!img.complete || !img.naturalWidth) return undefined;
  const size = 64;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return undefined;
  ctx.drawImage(img, 0, 0, size, size);
  let sum = 0;
  try {
    const { data } = ctx.getImageData(0, 0, size, size);
    for (let i = 0; i < data.length; i += 4) {
      sum += 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
    }
  } catch {
    return undefined; // a tainted canvas is not worth an exception
  }
  const mean = sum / (size * size);
  luma.set(src, mean);
  return mean;
}
