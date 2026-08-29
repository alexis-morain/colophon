// How wide a set line is, in the face the PDF embeds.
//
// The engine measures through `font::Embarquee`, reading the advance widths
// out of the very bytes it puts in the file. This side asks the browser to
// measure **those same bytes**, handed over by a command and registered
// under a family name of our own. That is what makes the overflow warning,
// the scene's ink rectangles and the print agree on every glyph instead of
// on an average one.
//
// **Never `font-family: "Helvetica Neue"`.** Naming an installed face works
// on the machine that has it, which is exactly what makes the defect
// invisible: the album measures right here and wrong everywhere else, and
// the installed face's kerning shifts the measure by a hair even here — the
// face copied beside the album has no kerning tables at all, the engine
// reads none, and the PDF draws none. Same bytes, same answer, on any
// machine. The family below is internal on purpose: it names the album's
// face, not a font anybody has installed.
//
// It lives alone, away from `scene.ts`, because it is the one piece of the
// scene that needs a document: Vitest runs without one, so the assembler
// takes the measure as an argument and this module is what the application
// hands it.

/** The album's face, whatever it turns out to be. Internal: nothing on any
 *  machine is called this, so the stack below can only resolve to the bytes
 *  we registered — or, until they land, to the face the engine ships. */
export const FAMILLE = "colophon-album";

/** The one stack, used by the canvas here and by `--font-book` in the
 *  stylesheet. The second entry is not a fallback anybody should reach: it
 *  covers the milliseconds before the album's face is registered, and it is
 *  the engine's own face rather than a system serif, so a caption measured
 *  in that window is off by a rounding rather than by a font. */
const PILE = `"${FAMILLE}", "Source Sans 3", sans-serif`;

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
      // The engine maps each character to its glyph and adds up advances,
      // full stop. A browser kerns by default, so without this the screen
      // would measure a line the printer will never set.
      if (ctx) ctx.fontKerning = "none";
    }
    return ctx;
  };
})();

/** Registered face, kept so a second choice can replace the first: two
 *  faces under one family name would leave the browser to pick. */
let posee: FontFace | null = null;

/** Resolves when the album's face is measurable. Replaced whole on every
 *  change, so a caller that awaited the old one is not left holding it. */
let pret: Promise<void> = Promise.resolve();

/** Bumped every time the face changes, for the views that have to remeasure
 *  what they already drew. */
let tour = 0;
const abonnes = new Set<() => void>();

/**
 * Register the album's face from the bytes the engine will embed.
 *
 * The face is registered without kerning or ligatures, because the engine
 * draws neither: it walks the character map and adds up advances. A face
 * copied beside an album carries no layout tables anyway — extraction drops
 * them — so this only matters for the face the engine ships, and there it
 * closes the last gap between what the screen shows and what prints.
 */
export async function chargerFace(octets: ArrayBuffer): Promise<void> {
  if (typeof document === "undefined") return;
  const face = new FontFace(FAMILLE, octets, {
    featureSettings: '"liga" 0, "clig" 0, "kern" 0',
  });
  pret = face
    .load()
    .then((f) => {
      if (posee) document.fonts.delete(posee);
      posee = f;
      document.fonts.add(f);
    })
    .then(
      () => undefined,
      // A face the browser refuses is not a reason to show nothing: the
      // stack falls back to the engine's own face, which is what the PDF
      // would embed if this one were unusable too.
      () => undefined,
    );
  await pret;
  tour += 1;
  abonnes.forEach((cb) => cb());
}

/** Subscribe to face changes, `useSyncExternalStore` style. */
export function surLaFace(cb: () => void): () => void {
  abonnes.add(cb);
  return () => {
    abonnes.delete(cb);
  };
}


/** Which face is registered, as a number that only ever grows. */
export function tourDeFace(): number {
  return tour;
}

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
  ctx.font = `100px ${PILE}`;
  return (ctx.measureText(text).width * sizeMm) / 100;
}

/**
 * Resolves once the album's face is loaded and measurements stop being
 * fallback guesses. It is a handful of bytes over an IPC call, so this is
 * milliseconds — but the first render happens inside them, and a caption
 * measured against another face would flag an overflow that does not exist.
 */
export function fontLoaded(): Promise<void> {
  if (typeof document === "undefined") return Promise.resolve();
  return pret.then(
    () => document.fonts.load(`100px "${FAMILLE}"`).then(
      () => undefined,
      () => undefined,
    ),
    () => undefined,
  );
}

// La poignée que `scripts/police-cdp.mjs` attrape, et la seule façon de
// mesurer dans le navigateur ce que le module mesure vraiment — plutôt
// qu'une réimplémentation dans le pilote, qui prouverait que le pilote sait
// mesurer. Attachée plutôt qu'exportée, et en dev seulement, sur le modèle
// de `mesure.ts` : le bundle n'en porte rien.
//
// La seconde poignée est le mordant lui-même : elle mesure la même chaîne en
// nommant une police installée, ce que ce module ne doit jamais faire. Si
// les deux répondent pareil, c'est que la face de l'album n'est pas chargée
// et que la comparaison ne prouve rien.
if (import.meta.env.DEV && typeof window !== "undefined") {
  const w = window as unknown as Record<string, unknown>;
  w.__mesureMm = measureMm;
  w.__mesureMmAvec = (
    pile: string,
    texte: string,
    mm: number,
    crenage: CanvasFontKerning = "none",
  ): number => {
    const ctx = contexte();
    if (!ctx) return 0;
    ctx.font = `100px ${pile}`;
    ctx.fontKerning = crenage;
    const large = (ctx.measureText(texte).width * mm) / 100;
    ctx.font = `100px ${PILE}`;
    ctx.fontKerning = "none";
    return large;
  };

}

