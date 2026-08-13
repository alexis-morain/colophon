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
}
