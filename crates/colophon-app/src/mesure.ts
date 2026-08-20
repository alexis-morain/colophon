// Render timings, taken before the canvas port so that after it there is
// something to compare against. A rewrite nobody measured is a bet.
//
// Three numbers, and they are all screen numbers: porting the renderer cannot
// move the engine, so timing a composition would measure the wrong thing.
//
//   planche.premiere   an album opens → its first spread is painted
//   planche.suivante   the reader moves on → the next spread is painted
//   recadrage.trame    one frame of a crop drag, pointer event → painted
//
// **Dev only.** Every entry point below compiles to a no-op branch on
// `import.meta.env.DEV`, so the bundle carries no measurement and no timer.
// The bundle is measured by hand, once, the way `scripts/mesure-rendu.md`
// says — and only the before/after *ratio* is load-bearing, because the dev
// server is slower than the bundle in a way that cancels out and a raw
// millisecond count from it would flatter or slander nothing in particular.

const ACTIF = import.meta.env.DEV;

const series = new Map<string, number[]>();

/** Longest run kept per series: enough for a stable median, bounded so a
 *  window left open all afternoon does not grow without end. */
const PLAFOND = 500;

function note(nom: string, ms: number): void {
  const s = series.get(nom) ?? [];
  s.push(ms);
  if (s.length > PLAFOND) s.shift();
  series.set(nom, s);
}

/**
 * Start a measure. The returned function closes it **on the frame after the
 * next paint**: `requestAnimationFrame` fires before the browser paints, so a
 * single one would stop the clock on work not yet on screen. Two nested ones
 * land after it.
 *
 * A window in the background never gets an animation frame — the same trap
 * the faithful preview already carries — so a measure begun there simply
 * never closes, and never lands a wrong number either.
 */
export function jusquAuRendu(nom: string): () => void {
  if (!ACTIF) return () => {};
  const t0 = performance.now();
  let ferme = false;
  return () => {
    if (ferme) return;
    ferme = true;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => note(nom, performance.now() - t0)),
    );
  };
}

/** One instantaneous sample, when the caller already holds both instants. */
export function echantillon(nom: string, ms: number): void {
  if (ACTIF) note(nom, ms);
}

function quantile(v: number[], q: number): number {
  const s = [...v].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.max(0, Math.round((s.length - 1) * q)))];
}

export type Releve = Record<
  string,
  { n: number; median: number; p95: number; max: number }
>;

/** Everything measured so far, rounded to a hundredth of a millisecond. */
export function releve(): Releve {
  const out: Releve = {};
  const r = (v: number) => Math.round(v * 100) / 100;
  for (const [nom, v] of series) {
    if (v.length === 0) continue;
    out[nom] = {
      n: v.length,
      median: r(quantile(v, 0.5)),
      p95: r(quantile(v, 0.95)),
      max: r(Math.max(...v)),
    };
  }
  return out;
}

export function oublier(): void {
  series.clear();
}

// The handle the measuring procedure reaches for. Attached rather than
// exported so a console — or the browser harness — can read it without the
// module graph, and only in dev.
if (ACTIF && typeof window !== "undefined") {
  (window as unknown as Record<string, unknown>).__mesures = releve;
  (window as unknown as Record<string, unknown>).__mesuresOubli = oublier;
}
