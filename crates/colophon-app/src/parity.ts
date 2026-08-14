// The one comparison between the engine's geometry dump and the TypeScript
// port. Two callers share it: the /__dev/geometry endpoint (vite.config.ts)
// and the Vitest parity test, so the check itself cannot drift either.

import {
  captionAnchor,
  coverSheet,
  cropWindow,
  DosProfil,
  mediaCanvas,
  slotsFor,
  TEMPLATES,
  templateForCount,
} from "./album";

/** Page formats the parity run sweeps: every preset shape plus a free size. */
export const PARITY_FORMATS = [
  "carre-21",
  "carre-30",
  "portrait-a4",
  "paysage-a4",
  "240x180",
];

type Dump = {
  trim_mm: { w: number; h: number };
  bleed_mm: number;
  canvas: { w: number; h: number; margin: number; gutter: number };
  templates: Record<string, { slots: number[][]; caption: number[] }>;
  fallbacks?: Record<string, [string, number]>;
  crop_windows?: {
    rect: [number, number];
    image: [number, number];
    focal: [number, number];
    zoom: number;
    window: [number, number, number, number];
  }[];
  covers?: {
    profil: string;
    spreads: number;
    sheet: [number, number];
    spine: number;
    /** `[x, width]` of the back panel then the front one. */
    panels: [[number, number], [number, number]];
  }[];
};

/** The spine parameters of the profiles the dump sweeps, as the engine holds
 *  them. Here rather than fetched: the parity test runs without a window, and
 *  a profile whose coefficient changes has to break this file too. */
const PARITY_DOS: Record<string, { dos: DosProfil; ext: number; haut: number; bas: number }> = {
  cloudprinter: {
    dos: { mode: "calcule", mm_par_feuille: 0.22, constante_mm: 1.5 },
    ext: 3,
    haut: 3,
    bas: 3,
  },
  prodigi: { dos: { mode: "fourni" }, ext: 0, haut: 0, bas: 0 },
  lulu: {
    dos: { mode: "calcule", mm_par_feuille: 0.2, constante_mm: 0 },
    ext: 3,
    haut: 3,
    bas: 3,
  },
  generique: { dos: { mode: "fourni" }, ext: 3, haut: 3, bas: 3 },
};

const near = (a: number, b: number) => Math.abs(a - b) < 1e-6;

/** Every disagreement between the dump and the port, as human-readable lines. */
export function geometryProblems(dump: Dump, label: string): string[] {
  const problems: string[] = [];
  const album = { trim_mm: dump.trim_mm, bleed_mm: dump.bleed_mm } as Parameters<
    typeof mediaCanvas
  >[0];
  const g = mediaCanvas(album);

  for (const key of ["w", "h", "margin", "gutter"] as const) {
    if (!near(g[key], dump.canvas[key])) {
      problems.push(`${label} canvas.${key}: rust ${dump.canvas[key]}, ts ${g[key]}`);
    }
  }

  for (const [name, want] of Object.entries(dump.templates)) {
    const n = want.slots.length;
    // The port works top-down; flip it back to compare with the PDF.
    const got = slotsFor(name, n, g).map((r) => [r.x, g.h - (r.y + r.h), r.w, r.h]);
    if (got.length !== n) {
      problems.push(`${label} ${name}: rust ${n} slots, ts ${got.length}`);
      continue;
    }
    want.slots.forEach((slot, i) => {
      slot.forEach((v, k) => {
        if (!near(v, got[i][k])) {
          problems.push(
            `${label} ${name} slot ${i}[${"xywh"[k]}]: rust ${v}, ts ${got[i][k]}`,
          );
        }
      });
    });

    const anchor = captionAnchor(name, n, g);
    const tsCaption = [anchor.x, g.h - anchor.y];
    want.caption.forEach((v, k) => {
      if (!near(v, tsCaption[k])) {
        problems.push(
          `${label} ${name} caption[${"xy"[k]}]: rust ${v}, ts ${tsCaption[k]}`,
        );
      }
    });
  }

  // The template list and the fallback rule are written twice too.
  for (const [name, cap] of TEMPLATES) {
    const want = dump.templates[name];
    if (!want) problems.push(`${label} ${name}: unknown to rust`);
    else if (want.slots.length !== cap) {
      problems.push(`${label} ${name} capacity: rust ${want.slots.length}, ts ${cap}`);
    }
  }
  for (const [n, want] of Object.entries(dump.fallbacks ?? {})) {
    const got = templateForCount(Number(n));
    if (!got || got[0] !== want[0] || got[1] !== want[1]) {
      problems.push(
        `fallback(${n}): rust ${JSON.stringify(want)}, ts ${JSON.stringify(got)}`,
      );
    }
  }

  // The cover sheet: the editor draws it and the printer receives it, from
  // one set of profile coefficients. A drift here ships a spine of the wrong
  // width, which is a reprint and not a redraw.
  for (const c of dump.covers ?? []) {
    const p = PARITY_DOS[c.profil];
    if (!p) {
      problems.push(`cover ${c.profil}: profil inconnu du port`);
      continue;
    }
    const got = coverSheet(
      { trim_mm: dump.trim_mm, spreads: new Array(c.spreads).fill(null) },
      { dos: p.dos, bleed_mm: { haut: p.haut, bas: p.bas, exterieur: p.ext } },
    );
    const tag = `${label} couverture ${c.profil} ${c.spreads}pl`;
    if (!near(got.w, c.sheet[0])) {
      problems.push(`${tag} largeur: rust ${c.sheet[0]}, ts ${got.w}`);
    }
    if (!near(got.h, c.sheet[1])) {
      problems.push(`${tag} hauteur: rust ${c.sheet[1]}, ts ${got.h}`);
    }
    if (!near(got.spine?.[1] ?? 0, c.spine)) {
      problems.push(`${tag} dos: rust ${c.spine}, ts ${got.spine?.[1] ?? 0}`);
    }
    const panels: [number, number][] = [got.back, got.front];
    c.panels.forEach((want, i) => {
      want.forEach((v, k) => {
        if (!near(v, panels[i][k])) {
          problems.push(
            `${tag} panneau ${i}[${"xw"[k]}]: rust ${v}, ts ${panels[i][k]}`,
          );
        }
      });
    });
  }

  // The manual-crop arithmetic (focal + zoom) exists on both sides too.
  for (const s of dump.crop_windows ?? []) {
    const got = cropWindow(
      { w: s.rect[0], h: s.rect[1] },
      s.image[0],
      s.image[1],
      s.focal,
      s.zoom,
    );
    s.window.forEach((v, k) => {
      if (!near(v, got[k])) {
        problems.push(
          `crop(zoom ${s.zoom}, focal ${s.focal})[${"xywh"[k]}]: rust ${v}, ts ${got[k]}`,
        );
      }
    });
  }
  return problems;
}
