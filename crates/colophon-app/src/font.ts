// How wide a set line is, in the face the PDF embeds.
//
// The engine measures with `font::text_width_mm`, reading the advance widths
// out of the same OFL file it embeds in the PDF. This side asks the browser
// to measure the same face — which is why the overflow warning, the scene's
// ink rectangles and the print agree on every glyph instead of on an
// average one.
//
// It lives alone, away from `scene.ts`, because it is the one piece of the
// scene that needs a document: Vitest runs without one, so the assembler
// takes the measure as an argument and this module is what the application
// hands it.

/** The one canvas context of the process: creating one per call would make
 *  a caption measured a hundred times a hundred canvases. */
const contexte = (() => {
  let ctx: CanvasRenderingContext2D | null = null;
  let tried = false;
  return (): CanvasRenderingContext2D | null => {
    if (!tried) {
      tried = true;
      ctx =
        typeof document === "undefined"
          ? null
          : document.createElement("canvas").getContext("2d");
    }
    return ctx;
  };
})();

/**
 * Width of a string at a print size in millimetres, in spread millimetres.
 *
 * Measured at a large fixed size and scaled down: glyph advances are linear
 * in the type size, and measuring big keeps the browser's own rounding well
 * under a micron once divided back.
 */
export function measureMm(text: string, sizeMm: number): number {
  const ctx = contexte();
  if (!ctx) return 0;
  ctx.font = '100px "Source Sans 3", sans-serif';
  return (ctx.measureText(text).width * sizeMm) / 100;
}

/**
 * Resolves once the embedded face is loaded and measurements stop being
 * fallback-font guesses. It is a local file, so this is milliseconds — but
 * the first render happens inside them, and a caption measured against a
 * system serif would flag an overflow that does not exist.
 */
export function fontLoaded(): Promise<void> {
  if (typeof document === "undefined") return Promise.resolve();
  return document.fonts.load('100px "Source Sans 3"').then(
    () => undefined,
    () => undefined,
  );
}
