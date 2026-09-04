// The one comparison between the engine's own output and what the editor
// still computes by itself. Two callers share it: the /__dev/geometry
// endpoint (vite.config.ts) and the Vitest parity test, so the check itself
// cannot drift either.
//
// Since the templates became data consumed from the dump, the slots are no
// longer a port: what this file guards is the algorithmic residue (crop
// windows, cover sheet, half-title layout under a synthetic measure), the
// dump-reading code itself (flip, truncation, per-count captions), and — as
// of the scene port — the order, the roles and the line breaking of what a
// spread holds.

import {
  captionAnchor,
  CAPTION_SIZE_MM,
  coverSheet,
  cropWindow,
  DosProfil,
  gardeLayout,
  PHOTO_CAPTION_SIZE_MM,
  Spread,
  spreadGeometry,
  SpreadGeometry,
  slotsFor,
  templateForCount,
  templates,
} from "./album";
import { Dump, setGeometrie } from "./geometrie";
import { decalage, replier, Scene, SceneObject, sceneOf } from "./scene";

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
    typeof spreadGeometry
  >[0];
  const g = spreadGeometry(album);

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

  // Le repli d'un bloc libre, rejoué sous la même mesure : c'est le seul
  // endroit où les deux ports décident ensemble d'une coupure de ligne, et
  // une coupure qui divergerait mettrait une ligne à l'écran et une autre
  // dans le PDF.
  for (const sample of dump.libre_samples ?? []) {
    // **La conversion du harnais, pas celle du moteur, et c'est voulu.**
    // `mesureSynthetique` rend `n × (tailleMm ÷ PT_MM) × 0,2` là où le moteur
    // rend `n × taille_pt × 0,2` : les deux ne sont le même nombre que si la
    // taille en millimètres arrive convertie par le `PT_MM` d'ici. Y verser
    // la conversion exacte (25,4 ÷ 72) introduirait dans la comparaison une
    // conversion que le moteur n'a jamais faite, et ferait dériver les deux
    // côtés d'un millionième — mesuré, pas supposé : 7,9 contre 7,900008.
    //
    // Ce qui est sous test ici est l'algorithme — où la ligne se coupe, où
    // elle se pose — pas l'arrondi d'une constante. En production `scene.ts`
    // convertit avec `album.ts::PT_MM`, qui est le 25,4 ÷ 72 du moteur.
    const tailleMm = sample.taille_pt * PT_MM;
    const got = replier(sample.texte, sample.largeur, tailleMm, mesureSynthetique);
    const tag = `${label} bloc libre « ${sample.texte.split("\n")[0]} »`;
    if (got.lignes.length !== sample.lignes.length) {
      problems.push(
        `${tag}: rust ${sample.lignes.length} lignes, ts ${got.lignes.length}` +
          ` (${JSON.stringify(got.lignes)})`,
      );
      continue;
    }
    sample.lignes.forEach((l, i) => {
      if (l !== got.lignes[i]) {
        problems.push(`${tag} ligne ${i}: rust « ${l} », ts « ${got.lignes[i]} »`);
      }
      // Et où la ligne se pose dans sa boîte : l'alignement est calculé sur
      // la même mesure, donc il se compare ici et pas dans la fixture.
      const dx = decalage(sample.alignement, sample.largeur, mesureSynthetique(l, tailleMm));
      if (!near(sample.dx[i], dx)) {
        problems.push(`${tag} ligne ${i} décalage: rust ${sample.dx[i]}, ts ${dx}`);
      }
    });
    if (sample.trop_large !== got.tropLarge) {
      problems.push(
        `${tag}: rust trop_large=${sample.trop_large}, ts ${got.tropLarge}`,
      );
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

// ---- the scene ----------------------------------------------------------
// `scene.ts` assembles, spread by spread, what `core::scene` assembles for
// the PDF. It declares no dimension of its own — every rectangle and anchor
// it uses comes from the dump — so what can drift is an order, a role, a
// reading rank or a line break. That is what the golden fixture pins, and
// what this compares, object by object, on the six page shapes.

/** One object as the engine serialises it: millimetres, origin bottom-left
 *  of the media box, type sizes in points. */
type ObjetMoteur = {
  rect: { x: number; y: number; w: number; h: number };
  /** Absent quand il vaut zéro, comme le moteur le sérialise. */
  angle?: number;
  reading: number;
  role: Record<string, any>;
};

/**
 * Millimetre tolerances, and why there are two.
 *
 * A photograph's rectangle is a dump lookup on both sides, so the two agree
 * to the last bit. A text box is built from a type size the dump rounds to
 * six decimals (`PT_MM` above, against the engine's own 25.4 ⁄ 72), which
 * moves its top edge by a hundredth of a micron. Neither number is a
 * tolerance for a real disagreement: a millimetre out of a thousand would
 * still be a hundred thousand times too big to see.
 */
const NEAR_MM = 1e-9;
const NEAR_TEXTE_MM = 1e-4;

/**
 * Every disagreement between one committed scene and the port, as readable
 * lines. `fixture` is the engine's `--dump-scene` for this page shape.
 *
 * **The measured ink is the one thing this cannot reproduce.** A width in
 * the embedded face needs the face, and Vitest runs without a document — so
 * both sides run the synthetic measure the half-title samples already use,
 * and the ink width is checked to be exactly what that measure returned,
 * rather than compared against a number obtained from a font file. The
 * browser hands `font.ts::measureMm` in its place, and the widths then are
 * the print's own.
 */
export function sceneProblems(
  fixture: unknown,
  spreads: Spread[],
  g: SpreadGeometry,
  label: string,
): string[] {
  const problems: string[] = [];
  const scenes = fixture as { objects: ObjetMoteur[] }[];
  if (!Array.isArray(scenes) || scenes.length !== spreads.length) {
    return [
      `${label}: ${spreads.length} planches, ${
        Array.isArray(scenes) ? scenes.length : "aucune"
      } scènes dans la fixture`,
    ];
  }

  // Bottom-up to top-left, the one conversion this file performs.
  const flipY = (y: number) => g.h - y;
  const flipRect = (r: ObjetMoteur["rect"]) => ({
    x: r.x,
    y: g.h - (r.y + r.h),
    w: r.w,
    h: r.h,
  });

  scenes.forEach((attendu, i) => {
    const tag = `${label} planche ${i + 1}`;
    const scene: Scene = sceneOf(spreads[i], g, mesureSynthetique);
    if (scene.objects.length !== attendu.objects.length) {
      problems.push(
        `${tag}: moteur ${attendu.objects.length} objets, port ${scene.objects.length}` +
          ` (${scene.objects.map((o) => o.role.role).join(", ") || "aucun"})`,
      );
      return;
    }
    attendu.objects.forEach((want, k) =>
      problems.push(
        ...objetProblems(`${tag} objet ${k}`, want, scene.objects[k], flipY, flipRect),
      ),
    );
  });

  return problems;
}

function objetProblems(
  tag: string,
  want: ObjetMoteur,
  got: SceneObject,
  flipY: (y: number) => number,
  flipRect: (r: ObjetMoteur["rect"]) => { x: number; y: number; w: number; h: number },
): string[] {
  const p: string[] = [];
  const ecart = (quoi: string, a: number, b: number, tol: number) => {
    if (!(Math.abs(a - b) < tol)) p.push(`${tag} ${quoi}: moteur ${a}, port ${b}`);
  };
  const egal = (quoi: string, a: unknown, b: unknown) => {
    if (JSON.stringify(a) !== JSON.stringify(b)) {
      p.push(`${tag} ${quoi}: moteur ${JSON.stringify(a)}, port ${JSON.stringify(b)}`);
    }
  };

  egal("rôle", want.role.role, got.role.role);
  if (want.role.role !== got.role.role) return p;
  egal("rang de lecture", want.reading, got.reading);
  // L'angle est le même nombre des deux côtés : il n'est pas converti, il est
  // porté tel que le moteur le stocke, et c'est un renderer qui le retourne
  // une fois. Absent dans la fixture veut dire zéro.
  egal("angle", want.angle ?? 0, got.angle);

  const wr = flipRect(want.rect);
  // A photograph's rectangle is the dump's, whole; a text box is measured,
  // so its width is checked against the measure and the rest against the
  // engine. A free object's box is neither: it is *stored*, so all four of
  // its numbers must match, and the only slack is the point-to-millimetre
  // conversion the two sides write differently.
  const photo = got.role.role === "photo";
  const libre = got.role.role === "free_text";
  const tol = photo ? NEAR_MM : NEAR_TEXTE_MM;
  ecart("rect.x", wr.x, got.rect.x, tol);
  ecart("rect.y", wr.y, got.rect.y, tol);
  ecart("rect.h", wr.h, got.rect.h, tol);
  if (photo || libre) ecart("rect.w", wr.w, got.rect.w, tol);

  switch (got.role.role) {
    case "photo": {
      egal("case", want.role.cell, got.role.cell);
      egal("source", want.role.src, got.role.src);
      egal("point focal", want.role.focal, got.role.focal);
      egal("zoom", want.role.zoom, got.role.zoom);
      break;
    }
    case "photo_caption":
    case "chapter_caption": {
      if (got.role.role === "photo_caption") egal("case", want.role.cell, got.role.cell);
      egal("texte", want.role.text, got.role.text);
      ecart("ancre.x", want.role.at.x, got.role.at.x, NEAR_MM);
      ecart("ancre.y", flipY(want.role.at.y), got.role.at.y, NEAR_MM);
      const taille =
        got.role.role === "photo_caption" ? PHOTO_CAPTION_SIZE_MM : CAPTION_SIZE_MM;
      ecart(
        "encre mesurée",
        mesureSynthetique(got.role.text, taille),
        got.rect.w,
        NEAR_MM,
      );
      break;
    }
    case "text": {
      ecart("ancre.x", want.role.at.x, got.role.at.x, NEAR_MM);
      ecart("ancre.y", flipY(want.role.at.y), got.role.at.y, NEAR_MM);
      const lignes = want.role.lines as { text: string; size_pt: number; dy_mm: number }[];
      if (lignes.length !== got.role.lines.length) {
        p.push(`${tag}: moteur ${lignes.length} lignes, port ${got.role.lines.length}`);
        break;
      }
      lignes.forEach((l, i) => {
        const w = got.role.role === "text" ? got.role.lines[i] : null;
        if (!w) return;
        egal(`ligne ${i}`, l.text, w.text);
        ecart(`ligne ${i} taille`, l.size_pt * PT_MM, w.sizeMm, NEAR_TEXTE_MM);
        ecart(`ligne ${i} dy`, l.dy_mm, w.dyMm, NEAR_MM);
      });
      ecart(
        "encre mesurée",
        Math.max(...got.role.lines.map((l) => mesureSynthetique(l.text, l.sizeMm))),
        got.rect.w,
        NEAR_MM,
      );
      break;
    }
    // **Ce que la fixture peut et ne peut pas dire d'un objet libre.**
    //
    // Sa boîte, son angle, son index, son alignement et sa première ligne de
    // base sont stockés ou dérivés de la boîte : aucune mesure ne les bouge,
    // donc ils se comparent ici, exactement.
    //
    // Où ses lignes se coupent, en revanche, dépend de la face — la fixture
    // est écrite dans la face du projet, la parité tourne sans face. Comparer
    // les lignes ici mesurerait la fonte, pas le port. C'est `libre_samples`
    // qui les tient, rejouées des deux côtés sous une mesure que ni l'un ni
    // l'autre ne possède, et `overflow` avec elles puisqu'il compte les
    // lignes produites.
    case "free_text": {
      egal("index", want.role.index, got.role.index);
      egal("alignement", want.role.align, got.role.align);
      ecart("ancre.x", want.role.at.x, got.role.at.x, NEAR_MM);
      ecart("ancre.y", flipY(want.role.at.y), got.role.at.y, NEAR_TEXTE_MM);
      // Ce qui se vérifie sans la face : le pas vertical est régulier et
      // vaut l'interlignage, ligne après ligne. Une ligne qui glisserait
      // d'un cran ne se verrait pas dans un texte comparé mot à mot, elle se
      // voit ici.
      const lignes = got.role.lines;
      if (lignes.length > 2) {
        const pas = lignes[1].dyMm - lignes[0].dyMm;
        lignes.forEach((l, i) => {
          ecart(`ligne ${i} dy`, i * pas, l.dyMm, NEAR_MM);
        });
      }
      break;
    }
  }
  return p;
}
