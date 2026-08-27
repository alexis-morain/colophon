// The faithful preview: the spread as the PDF holds it, drawn by pdf.js.
//
// The editor draws in the DOM and the press reads a PDF. Those two can never
// be identical by construction, and the gap between them is the first
// objection anybody technical raises about an album maker. So the preview
// stops being a second renderer and becomes a reader of the one that counts:
// same file, same page, same glyphs, same crops.
//
// It is a mode, not a replacement. Editing needs a DOM, so ⌘1 keeps drawing
// the spread; the eye button beside the ruler shows what will print. Leaving
// the mode costs nothing, entering it costs one PDF render when the album has
// moved since the last one.
//
// The bitmaps themselves live in `raster.ts`: two views need them, and the
// sheet that turns needs the same page in two places at once.

import { useEffect, useRef, useState } from "react";
import { Feuilletage, Tourneur } from "./Feuilletage";
import { t } from "./i18n";
import { Quoi, ouvrir } from "./raster";

/**
 * One page of one of the album's PDFs, drawn to a canvas at the device's own
 * resolution. `page` is 1-based, the way the file numbers its pages.
 */
export function PdfPage({
  quoi,
  page,
  cle,
  largeur,
  onErreur,
}: {
  quoi: Quoi;
  page: number;
  /** Bumped whenever the file on disk is re-rendered. */
  cle: number;
  /** Width to draw at, in CSS pixels. The height follows the page. */
  largeur: number;
  onErreur?: (message: string) => void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  // Held in a ref rather than read from the deps: an inline callback changes
  // identity at every render of the window, and the status line alone
  // re-renders it every few seconds. In the deps it would cancel the render
  // in flight each time, and the page would never finish drawing.
  const erreur = useRef(onErreur);
  erreur.current = onErreur;

  useEffect(() => {
    let vivant = true;
    let job: { promise: Promise<void>; cancel(): void } | null = null;
    (async () => {
      const doc = await ouvrir(quoi, cle);
      if (!vivant) return;
      const n = Math.min(Math.max(page, 1), doc.numPages);
      const p = await doc.getPage(n);
      if (!vivant || !canvas.current) return;
      // Draw at the screen's real pixels: a spread at 800 CSS pixels on a
      // retina display is 1600 of them, and half of that is a blurred page
      // that would be blamed on the PDF.
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const unite = p.getViewport({ scale: 1 });
      const viewport = p.getViewport({ scale: (largeur * dpr) / unite.width });
      const ctx = canvas.current.getContext("2d");
      if (!ctx) return;
      canvas.current.width = Math.round(viewport.width);
      canvas.current.height = Math.round(viewport.height);
      canvas.current.style.width = `${largeur}px`;
      canvas.current.style.height = `${Math.round(viewport.height / dpr)}px`;
      job = p.render({ canvasContext: ctx, viewport });
      try {
        await job.promise;
      } catch (e) {
        // A cancelled render is the normal way to leave a page, not a fault.
        if (vivant && !String(e).includes("ancel")) erreur.current?.(String(e));
      }
    })().catch((e) => vivant && erreur.current?.(String(e)));
    return () => {
      vivant = false;
      // pdf.js keeps painting a page nobody is looking at otherwise, and a
      // hundred-spread album flipped through fast would queue a hundred.
      job?.cancel();
    };
  }, [quoi, page, cle, largeur]);

  return (
    <canvas
      ref={canvas}
      className="pdf-page"
      aria-label={t("fidele.pdf.aria")}
    />
  );
}

/**
 * The faithful preview inside the book view's stage: the page fitted to the
 * room available, on the same paper-coloured ground as the DOM spread so
 * switching between the two compares like with like.
 *
 * It measures the room and hands the width on; what stands in it depends on
 * where the reader is. The interior is a run of sheets, and sheets turn, so
 * it goes to `Feuilletage`. The cover is its own file, a flat sheet with the
 * spine in the middle: one page, no fold to turn about, and the interior's
 * page numbering does not apply to it — it stays a plain drawn page.
 */
export function ApercuFidele({
  onCover,
  page,
  total,
  cle,
  album,
  onPlanche,
  onErreur,
  ref,
}: {
  onCover: boolean;
  /** 1-based page of the interior PDF, one page per spread. */
  page: number;
  /** How many spreads the album has. */
  total: number;
  cle: number;
  album: { trim_mm: { w: number; h: number } };
  /** Commits a turn the sheet finished. Returns false when there is none. */
  onPlanche: (sens: number) => boolean;
  onErreur: (message: string) => void;
  ref?: React.Ref<Tourneur | null>;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [largeur, setLargeur] = useState(0);

  // The stage resizes with the window; the canvas has to be redrawn at the
  // new size rather than stretched, or the faithful preview would be the
  // blurriest thing on screen.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    // Measured on the stage, never on this element: the canvas lives inside
    // it, so measuring itself would feed its own size back and the page
    // would grow at every observation until it ran off the window. The stage
    // carries `container-type: size`, so its box does not depend on what it
    // holds.
    const scene = el.closest(".stage") as HTMLElement | null;
    const cible = scene ?? el;
    const mesure = () => {
      const style = getComputedStyle(cible);
      const rect = {
        width:
          cible.clientWidth -
          parseFloat(style.paddingLeft) -
          parseFloat(style.paddingRight),
        height:
          cible.clientHeight -
          parseFloat(style.paddingTop) -
          parseFloat(style.paddingBottom),
      };
      // Fit the sheet: a spread is twice as wide as a page, the cover a
      // little wider still with its spine. Height is the binding constraint
      // on a square format, width on a landscape one.
      const ratio = onCover
        ? (album.trim_mm.w * 2.1) / album.trim_mm.h
        : (album.trim_mm.w * 2) / album.trim_mm.h;
      setLargeur(Math.max(0, Math.floor(Math.min(rect.width, rect.height * ratio))));
    };
    mesure();
    const ro = new ResizeObserver(mesure);
    ro.observe(cible);
    return () => ro.disconnect();
  }, [onCover, album.trim_mm.w, album.trim_mm.h]);

  return (
    <div className="pdf-stage" ref={box}>
      {largeur > 0 &&
        (onCover ? (
          <PdfPage
            quoi="couverture"
            page={1}
            cle={cle}
            largeur={largeur}
            onErreur={onErreur}
          />
        ) : (
          <Feuilletage
            planche={page - 1}
            total={total}
            cle={cle}
            largeur={largeur}
            onPlanche={onPlanche}
            onErreur={onErreur}
            ref={ref}
          />
        ))}
    </div>
  );
}
