// Le PDF de l'album, en bitmaps : pdf.js chargé une fois, les documents
// ouverts une fois, et les pages dessinées une fois chacune.
//
// Séparé des composants parce qu'ils sont deux à en dépendre — la page à plat
// et la feuille qui tourne — et qu'une dépendance croisée entre deux fichiers
// de vue est une manière lente de découvrir qu'on avait un module.

import { albumPdfBytes } from "./bridge";

/** pdf.js and its worker, loaded once, on the first faithful preview. The
 *  library is a megabyte: nobody who never opens the preview pays for it. */
let pdfjs: Promise<typeof import("pdfjs-dist")> | null = null;

async function lib() {
  if (!pdfjs) {
    pdfjs = (async () => {
      const mod = await import("pdfjs-dist");
      // The worker travels in the bundle, never from a CDN: the app has to
      // work with no network at all, and the CSP forbids the other case.
      const url = (await import("pdfjs-dist/build/pdf.worker.mjs?url")).default;
      mod.GlobalWorkerOptions.workerSrc = url;
      return mod;
    })();
  }
  return pdfjs;
}

/** One opened document, per file, so flipping through the book does not
 *  reparse a hundred-megabyte PDF at every page. Dropped when the file is
 *  re-rendered, which is what `cle` changes for. */
const docs = new Map<string, { cle: number; doc: Promise<PdfDoc> }>();

/** Deliberately not called `document`: this module draws into canvases it
 *  creates itself now, and a local function of that name would shadow the
 *  global one at exactly the place where it is needed. */

/** Which of the album's two PDFs. A closed set, the same one the bridge
 *  accepts: this module never became a file reader. */
export type Quoi = "album" | "couverture";

export type PdfDoc = { numPages: number; getPage(n: number): Promise<PdfPageProxy> };
export type PdfPageProxy = {
  getViewport(o: { scale: number }): { width: number; height: number };
  render(o: {
    canvasContext: CanvasRenderingContext2D;
    viewport: { width: number; height: number };
  }): { promise: Promise<void>; cancel(): void };
};

export async function ouvrir(quoi: Quoi, cle: number): Promise<PdfDoc> {
  const hit = docs.get(quoi);
  if (hit && hit.cle === cle) return hit.doc;
  const doc = (async () => {
    const [mod, bytes] = await Promise.all([lib(), albumPdfBytes(quoi)]);
    return mod.getDocument({ data: new Uint8Array(bytes) }).promise as Promise<PdfDoc>;
  })();
  docs.set(quoi, { cle, doc });
  return doc;
}

/** Forget every opened document: called after a re-render, when the bytes on
 *  disk no longer match what is parsed in memory. */
export function forgetPdfs() {
  docs.clear();
  rasters.clear();
}

// ---- one page, drawn once, read from several places ----------------------
//
// A turning sheet needs the same spread in two places at once — half of it
// lies still while the other half swings — and a canvas element only exists
// in one. So the page is drawn into a canvas nobody mounts, and every visible
// half is a `drawImage` away from it: a blit, synchronous, off the same
// bitmap, which is what keeps the two faces of a sheet from disagreeing.

/** A page of a PDF, drawn at the device's resolution, with the CSS box it
 *  was drawn for. `source` is offscreen and shared: never mount it. */
export type Raster = {
  source: HTMLCanvasElement;
  /** CSS pixels, the width it was asked for. */
  largeur: number;
  /** CSS pixels, following the page's own shape. */
  hauteur: number;
};

type Entree = { promesse: Promise<Raster>; pret: Raster | null };

/**
 * Kept: the spread on screen, its two neighbours, and a little slack for a
 * reader going back and forth over the same fold. A spread at a thousand CSS
 * pixels on a retina screen is some ten megabytes of bitmap, so this is a
 * ceiling worth having rather than a cache worth growing.
 */
const PLAFOND = 5;

const rasters = new Map<string, Entree>();

function cleRaster(quoi: Quoi, page: number, cle: number, largeur: number) {
  return `${quoi}|${page}|${cle}|${Math.round(largeur)}`;
}

/** Drop the least recently asked for, down to the ceiling. */
function elaguer() {
  for (const k of rasters.keys()) {
    if (rasters.size <= PLAFOND) return;
    rasters.delete(k);
  }
}

async function dessiner(
  quoi: Quoi,
  page: number,
  cle: number,
  largeur: number,
): Promise<Raster> {
  const doc = await ouvrir(quoi, cle);
  const n = Math.min(Math.max(page, 1), doc.numPages);
  const p = await doc.getPage(n);
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const unite = p.getViewport({ scale: 1 });
  const viewport = p.getViewport({ scale: (largeur * dpr) / unite.width });
  const source = window.document.createElement("canvas");
  source.width = Math.round(viewport.width);
  source.height = Math.round(viewport.height);
  const ctx = source.getContext("2d");
  if (!ctx) throw new Error("2d");
  await p.render({ canvasContext: ctx, viewport }).promise;
  return { source, largeur, hauteur: Math.round(viewport.height / dpr) };
}

/** The page, drawn or being drawn. Asking twice costs one drawing. */
export function rasteriser(
  quoi: Quoi,
  page: number,
  cle: number,
  largeur: number,
): Promise<Raster> {
  const k = cleRaster(quoi, page, cle, largeur);
  const hit = rasters.get(k);
  if (hit) {
    // Re-inserted so the map's own order is the order last asked for.
    rasters.delete(k);
    rasters.set(k, hit);
    return hit.promesse;
  }
  const entree: Entree = { promesse: dessiner(quoi, page, cle, largeur), pret: null };
  entree.promesse.then(
    (r) => (entree.pret = r),
    () => rasters.delete(k),
  );
  rasters.set(k, entree);
  elaguer();
  return entree.promesse;
}

/**
 * The page if it is already drawn, and nothing otherwise — no promise, no
 * wait. A caller that has it can paint on the same frame it decided to, which
 * is the difference between a sheet that turns and a sheet that blinks.
 */
export function rasterPret(
  quoi: Quoi,
  page: number,
  cle: number,
  largeur: number,
): Raster | null {
  return rasters.get(cleRaster(quoi, page, cle, largeur))?.pret ?? null;
}

/**
 * Draw ahead the pages a gesture is about to need.
 *
 * **Never from a window in the background.** pdf.js settles its render
 * promise on an animation frame, and a hidden window is given none: the work
 * would sit half done and the cache would fill with promises that answer only
 * when the window comes back. The caller listens for `visibilitychange` and
 * asks again — which is also when preloading is worth anything.
 */
export function precharger(
  quoi: Quoi,
  pages: number[],
  cle: number,
  largeur: number,
): void {
  if (largeur <= 0) return;
  if (window.document.visibilityState !== "visible") return;
  for (const page of pages) {
    void rasteriser(quoi, page, cle, largeur).catch(() => {
      /* a page that will not draw is the reader's problem, not the cache's */
    });
  }
}
