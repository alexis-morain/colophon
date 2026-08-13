// The one comparison between the engine's geometry dump and the TypeScript
// port. Two callers share it: the /__dev/geometry endpoint (vite.config.ts)
// and the Vitest parity test, so the check itself cannot drift either.

import {
  captionAnchor,
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
  return problems;
}
