// The one comparison between the engine's geometry dump and what the editor
// still computes by itself. Two callers share it: the /__dev/geometry
// endpoint (vite.config.ts) and the Vitest parity test, so the check itself
// cannot drift either.
//
// Since the templates became data consumed from the dump, the slots are no
// longer a port: what this file guards is the algorithmic residue (crop
// windows, cover sheet, half-title layout under a synthetic measure) and
// the dump-reading code itself (flip, truncation, per-count captions).

import {
  captionAnchor,
  coverSheet,
  cropWindow,
  DosProfil,
  gardeLayout,
  mediaCanvas,
  slotsFor,
  templateForCount,
  templates,
} from "./album";
import { Dump, setGeometrie } from "./geometrie";

/** Page formats the parity run sweeps: every preset shape plus a free size. */
export const PARITY_FORMATS = [
  "carre-21",
  "carre-30",
  "portrait-a4",
  "paysage-a4",
  "240x180",
];

/** The spine parameters of the profiles the dump sweeps, as the engine holds
 *  them. Here rather than fetched: the parity test runs without a window, and
 *  a profile whose coefficient changes has to break this file too. */
const PARITY_DOS: Record<string, { dos: DosProfil; ext: number; haut: number; bas: number }> = {
  cloudprinter: {
    dos: { mode: "calcule", mm_par_feuille: 0.12, constante_mm: 6.0 },
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

/** The synthetic measure both sides run for the half-title samples: width
 *  grows with the glyph count, enough to exercise the shrink formula. The
 *  engine's copy lives in `dump_geometry`; the two must match. */
const PT_MM = 0.352778;
const mesureSynthetique = (s: string, tailleMm: number): number =>
  [...s].length * (tailleMm / PT_MM) * 0.2;

/** Every disagreement between the dump and the editor's remaining
 *  arithmetic, as human-readable lines. Installs the dump, then checks that
 *  the dump-fed lookups and the true ports agree with the engine. */
export function geometryProblems(dump: Dump, label: string): string[] {
  const problems: string[] = [];
  setGeometrie(dump);
  const album = { trim_mm: dump.trim_mm, bleed_mm: dump.bleed_mm } as Parameters<
    typeof mediaCanvas
  >[0];
  const g = mediaCanvas(album);

  // The dump-reading code: flip, truncation, captions per count. The values
  // come from the dump; what can drift is the reading of them.
  for (const [name, cap] of templates()) {
    const want = dump.templates[name];
    if (!want) {
      problems.push(`${label} ${name}: absent du dump`);
      continue;
    }
    const got = slotsFor(name, cap, g).map((r) => [r.x, g.h - (r.y + r.h), r.w, r.h]);
    if (got.length !== want.slots.length) {
      problems.push(`${label} ${name}: dump ${want.slots.length} slots, lecture ${got.length}`);
      continue;
    }
    want.slots.forEach((slot, i) => {
      slot.forEach((v, k) => {
        if (!near(v, got[i][k])) {
          problems.push(
            `${label} ${name} slot ${i}[${"xywh"[k]}]: dump ${v}, lecture ${got[i][k]}`,
          );
        }
      });
    });
    // A partially filled spread truncates, and its caption anchor moves.
    const partiel = Math.max(1, cap - 1);
    if (cap > 0 && slotsFor(name, partiel, g).length !== partiel) {
      problems.push(`${label} ${name}: la troncature à ${partiel} ne tronque pas`);
    }
    const anchor = captionAnchor(name, partiel, g);
    const wantAt = want.captions[Math.min(partiel, want.captions.length - 1)];
    if (!near(anchor.x, wantAt[0]) || !near(g.h - anchor.y, wantAt[1])) {
      problems.push(`${label} ${name} caption(${partiel}): dump ${wantAt}, lecture ${[anchor.x, g.h - anchor.y]}`);
    }
  }

  for (const [n, want] of Object.entries(dump.fallbacks)) {
    const got = templateForCount(Number(n));
    if (!got || got[0] !== want[0] || got[1] !== want[1]) {
      problems.push(
        `fallback(${n}): dump ${JSON.stringify(want)}, lecture ${JSON.stringify(got)}`,
      );
    }
  }

  // The half-title layout is still an algorithm here: replay the engine's
  // samples under the shared synthetic measure, shrink formula included.
  for (const sample of dump.garde_samples) {
    const got = gardeLayout(sample.texte, sample.place, mesureSynthetique);
    const tag = `${label} garde « ${sample.texte.split("\n")[0]} »`;
    if (got.length !== sample.lignes.length) {
      problems.push(`${tag}: rust ${sample.lignes.length} lignes, ts ${got.length}`);
      continue;
    }
    sample.lignes.forEach(([texte, taillePt, dyMm], i) => {
      const l = got[i];
      if (l.texte !== texte) problems.push(`${tag} ligne ${i}: « ${texte} » vs « ${l.texte} »`);
      if (!near(l.tailleMm, taillePt * PT_MM)) {
        problems.push(`${tag} ligne ${i} taille: rust ${taillePt * PT_MM}, ts ${l.tailleMm}`);
      }
      if (!near(l.dyMm, dyMm)) {
        problems.push(`${tag} ligne ${i} dy: rust ${dyMm}, ts ${l.dyMm}`);
      }
    });
  }

  // Crop windows: the drag arithmetic is written twice, and a drift here
  // silently shifts every recadrage between the preview and the print.
  for (const c of dump.crop_windows ?? []) {
    const got = cropWindow(
      { w: c.rect[0], h: c.rect[1] },
      c.image[0],
      c.image[1],
      c.focal,
      c.zoom,
    );
    c.window.forEach((v, k) => {
      if (!near(v, got[k])) {
        problems.push(
          `${label} crop ${JSON.stringify(c.rect)}@${c.zoom}[${k}]: rust ${v}, ts ${got[k]}`,
        );
      }
    });
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

  return problems;
}
