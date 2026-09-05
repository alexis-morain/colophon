// What one spread holds, as objects — the editor's half of `core::scene`.
//
// The engine derives a scene to emit the PDF, to size its JPEGs, to count
// defects and to measure the preflight; this side derives the same one to
// draw it. **The scene is not fetched.** Bringing it over the bridge would
// make every keystroke in a caption and every template change asynchronous,
// where `edits.ts` is pure and synchronous today — so the assembly is
// ported, under the regime `album.ts` already states: the algorithmic
// residue the dump cannot carry, each piece pinned by the parity test.
//
// It invents no dimension. Every rectangle, anchor and type size it uses
// comes from `album.ts`, which reads them from the engine's geometry dump.
// What is written here is the order of the objects, their roles, and where
// a block of text is cut into lines — and that is exactly what the golden
// fixture pins, spread by spread, on the six page shapes.
//
// **Two conventions differ from the Rust type, on purpose.** Rectangles are
// millimetres with the origin at the *top left* of the media box, like every
// other rectangle on this side (see `album.ts::Rect`); and type sizes are in
// millimetres, not points, because a PDF speaks points and a screen does
// not. The parity test converts the fixture once, in one place, and compares
// everything else without touching it.

import {
  Alignement,
  captionAnchor,
  CAPTION_SAFE,
  colophonAnchor,
  COLOPHON_LEADING_MM,
  COLOPHON_SIZE_MM,
  COLOPHON_TEMPLATE,
  CAPTION_SIZE_MM,
  gardeAnchor,
  gardeLayout,
  gardePlace,
  GARDE_TEMPLATE,
  interligneDe,
  Objet,
  PHOTO_CAPTION_DROP_MM,
  PHOTO_CAPTION_SIZE_MM,
  PT_MM,
  Rect,
  Spread,
  SpreadGeometry,
  slotsFor,
  textAnchor,
  TEXT_LEADING_MM,
  TEXT_SIZE_MM,
} from "./album";

/** Millimetres, origin top-left of the media box. */
export type Point = { x: number; y: number };

/** One line of a text block, already laid out: `dyMm` grows downward from
 *  the block's first baseline, `dxMm` rightward from its left edge.
 *
 *  `dxMm` carries the alignment of a free block, computed in the assembler
 *  rather than in each renderer: a line the canvas centred and the PDF did
 *  not would be a preview that lies. Zero for the three text pages, which
 *  have never been anything but left-aligned. */
export type SceneLine = {
  text: string;
  sizeMm: number;
  dyMm: number;
  dxMm: number;
};

/**
 * What an object is, as a code with the parameters an interface needs to
 * name it. Never a rendered sentence: a string born in the engine stays in
 * the language it was born in, and this side has to say "photo 3 of 4" in
 * two languages.
 */
export type Role =
  | {
      role: "photo";
      cell: number;
      src: string;
      focal: [number, number];
      zoom: number;
    }
  /** `at` is the baseline: a set line is placed by where it sits, and covers
   *  the ink it happens to cover; neither derives from the other. */
  | { role: "photo_caption"; cell: number; text: string; at: Point }
  | { role: "chapter_caption"; text: string; at: Point }
  /** The half-title, a text page, the colophon: one role, not three cases. */
  | { role: "text"; at: Point; lines: SceneLine[] }
  /** A block the reader placed and may have turned. `index` points into
   *  `spread.objets` the way `cell` points into `spread.slots`, and `at` is
   *  the first baseline in the object's own upright frame — the renderer
   *  turns the box once around its centre and draws upright inside it. */
  | {
      role: "free_text";
      index: number;
      at: Point;
      lines: SceneLine[];
      align: Alignement;
      /** Set text taller than its box. Signalled, never cut. */
      overflow: boolean;
      /** One word wider than the box: printed whole, past the edge. */
      tropLarge: boolean;
    };

/** One visible element: where it is, how it is turned, when it is read, and
 *  what it is.
 *
 *  **An angle and an origin, never a matrix**, mirroring `scene.rs::Object`.
 *  The angle is degrees counter-clockwise **in the engine's frame** around
 *  the rectangle's centre — the same number the engine stores, kept
 *  unconverted so the parity fixture compares like with like. A screen's y
 *  runs the other way, so a renderer negates it exactly once; `angleEcran`
 *  below is that once. */
export type SceneObject = {
  rect: Rect;
  angle: number;
  reading: number;
  role: Role;
};

/** The angle to hand a screen transform — a CSS `rotate()`, a canvas
 *  `ctx.rotate()` — for an object turned by `angle` in the engine's frame.
 *
 *  The whole y-flip of this file, for rotations, is this one negation. It is
 *  a function rather than a comment because three renderers need it and a
 *  fourth will. */
export function angleEcran(angle: number): number {
  return -angle;
}

/** The centre a turned object pivots around: the middle of its box. */
export function centre(r: Rect): Point {
  return { x: r.x + r.w / 2, y: r.y + r.h / 2 };
}

/** One point turned around another. Port of `scene.rs::tourner`, in the
 *  screen's frame — hence the sign, taken once from `angleEcran`. */
export function tourner(p: Point, c: Point, angle: number): Point {
  const t = (angleEcran(angle) * Math.PI) / 180;
  const [sin, cos] = [Math.sin(t), Math.cos(t)];
  const [dx, dy] = [p.x - c.x, p.y - c.y];
  return { x: c.x + dx * cos - dy * sin, y: c.y + dx * sin + dy * cos };
}

/** The four corners of an oriented rectangle. **The upright case returns the
 *  rectangle's own numbers**, for the reason `scene.rs::corners` gives:
 *  `(x + w/2) - w/2` is not `x` in binary floating point, and an upright
 *  object must measure exactly what it measured before the angle existed. */
export function corners(r: Rect, angle: number): Point[] {
  const coins = [
    { x: r.x, y: r.y },
    { x: r.x + r.w, y: r.y },
    { x: r.x + r.w, y: r.y + r.h },
    { x: r.x, y: r.y + r.h },
  ];
  if (angle === 0) return coins;
  const c = centre(r);
  return coins.map((p) => tourner(p, c, angle));
}

/** Whether an oriented box runs across the fold. Port of
 *  `scene.rs::traverse_le_pli`: the editor stops a gesture with this, because
 *  nothing has ever crossed the fold and a free object does not start. */
export function traverseLePli(
  r: Rect,
  angle: number,
  g: SpreadGeometry,
): boolean {
  const xs = corners(r, angle).map((p) => p.x);
  const pli = g.w / 2;
  return Math.min(...xs) < pli && Math.max(...xs) > pli;
}

/** Distance from an oriented box to the trimmed edge, in millimetres.
 *  Negative means the cut runs through it. Port of
 *  `scene.rs::distance_to_trim`, in the screen's frame, where the flip is
 *  invisible: the four trim lines are symmetric. */
export function distanceToTrim(
  r: Rect,
  angle: number,
  g: SpreadGeometry,
): number {
  return Math.min(
    ...corners(r, angle).map((p) =>
      Math.min(p.x - g.bleed, p.y - g.bleed, g.w - g.bleed - p.x, g.h - g.bleed - p.y),
    ),
  );
}

/** Where a line sits inside the box it was wrapped to, given its alignment.
 *  Port of `scene.rs::decalage`, and one function rather than a ternary
 *  copied into each renderer for the same reason the engine has one. */
export function decalage(
  align: Alignement,
  boite: number,
  ligne: number,
): number {
  if (align === "gauche") return 0;
  return align === "centre" ? (boite - ligne) / 2 : boite - ligne;
}

/**
 * Une boîte du repère de l'écran vers celui du moteur, ou l'inverse.
 *
 * Le moteur pose l'origine en bas à gauche, l'écran en haut à gauche — et le
 * retournement est **son propre inverse**, donc une seule fonction dit les
 * deux sens. C'est la conversion que `objetLibre` fait à l'aller ; celle-ci
 * est le retour, celui qu'un geste emprunte pour écrire dans `album.json`.
 */
export function retournerBoite(r: Rect, g: SpreadGeometry): Rect {
  return { ...r, y: g.h - (r.y + r.h) };
}

/** De quel côté du pli un objet se tient : -1 la page de gauche, 1 celle de
 *  droite. Pris au *début* d'un geste et gardé jusqu'à sa fin, pour qu'un
 *  glissement ne fasse jamais sauter un objet d'une page à l'autre. */
export type Cote = -1 | 1;

/** Le côté où se tient le centre d'une boîte. */
export function coteDe(r: Rect, g: SpreadGeometry): Cote {
  return r.x + r.w / 2 < g.w / 2 ? -1 : 1;
}

/**
 * Ramener une boîte du bon côté du pli, en la translatant.
 *
 * **Le pli est dur** : aucune image ne l'a jamais traversé, et un objet libre
 * ne commence pas. La butée est une translation et pas un refus, pour que le
 * geste continue de suivre la main au lieu de se figer — on bute, on ne casse
 * pas.
 *
 * Elle travaille sur les coins, donc sur l'objet tourné : une boîte qui
 * dégagerait le pli droite peut le franchir d'un coin une fois tournée.
 */
export function retenirAuPli(
  r: Rect,
  angle: number,
  g: SpreadGeometry,
  cote: Cote,
): Rect {
  const xs = corners(r, angle).map((p) => p.x);
  const pli = g.w / 2;
  const dx =
    cote === -1
      ? Math.min(0, pli - Math.max(...xs))
      : Math.max(0, pli - Math.min(...xs));
  return dx === 0 ? r : { ...r, x: r.x + dx };
}

/**
 * L'objet sort-il de la zone sûre ?
 *
 * **La marge est molle** : on avertit, on ne refuse pas. C'est la règle du
 * projet — tout défaut a une échappatoire manuelle — et c'est aussi la seule
 * tenable, puisqu'un objet délibérément posé à fond perdu est un choix, pas
 * une erreur. Le compteur de linter qui comptera ceux-là arrive en 6.4.
 *
 * La zone sûre est celle que le moteur ancre déjà pour les légendes :
 * `CAPTION_SAFE` de marge à l'intérieur de la coupe.
 *
 * Port de `scene.rs::hors_marge`, comme `traverseLePli` l'est de
 * `traverse_le_pli` : le compteur `objet_hors_marge` du linter lit la même
 * règle, et un seuil qui ne vivrait que de ce côté-ci ferait avertir l'écran
 * sur des objets que le rapport ne compterait pas.
 */
export function horsMarge(r: Rect, angle: number, g: SpreadGeometry): boolean {
  return distanceToTrim(r, angle, g) < CAPTION_SAFE * g.margin;
}

/**
 * Cut a text into the lines it sets as, inside a box `largeur` wide. Port of
 * `scene.rs::replier`, and the parity fixture is what keeps the two the same
 * function rather than two functions that agree today.
 *
 * A typed newline is a hard break and an empty paragraph keeps its turn.
 * **A word wider than the box goes on its line whole**: hyphenating is a
 * decision about someone's language, and the caller is told instead.
 */
export function replier(
  texte: string,
  largeur: number,
  tailleMm: number,
  measure: Measure,
): { lignes: string[]; tropLarge: boolean } {
  const lignes: string[] = [];
  let tropLarge = false;
  for (const para of texte.split("\n")) {
    const mots = para.split(" ").filter((m) => m !== "");
    if (mots.length === 0) {
      lignes.push("");
      continue;
    }
    let courante = "";
    for (const mot of mots) {
      if (courante === "") {
        courante = mot;
        continue;
      }
      const candidat = `${courante} ${mot}`;
      if (measure(candidat, tailleMm) <= largeur) {
        courante = candidat;
      } else {
        tropLarge ||= measure(courante, tailleMm) > largeur;
        lignes.push(courante);
        courante = mot;
      }
    }
    tropLarge ||= measure(courante, tailleMm) > largeur;
    lignes.push(courante);
  }
  return { lignes, tropLarge };
}

/** Everything visible on one spread, back to front. **The index is the
 *  depth**: object `n` paints over object `n - 1`, in the exact order the
 *  PDF's content stream lays them down. */
export type Scene = { objects: SceneObject[] };

/** Width of a string at a print size in millimetres. Passed in rather than
 *  imported so the assembler stays pure: the application hands it
 *  `font.ts::measureMm`, the parity test hands it the synthetic measure the
 *  engine also runs. */
export type Measure = (text: string, sizeMm: number) => number;

/** Ink of one set line: the measured width, and the vertical box the engine
 *  has always used around a baseline (`scene.rs::ink_box`, read top-down). */
function inkBox(
  x: number,
  baseline: number,
  text: string,
  sizeMm: number,
  measure: Measure,
): Rect {
  return {
    x,
    y: baseline - sizeMm * 1.05,
    w: measure(text, sizeMm),
    h: sizeMm * 1.35,
  };
}

/** The union of two boxes. */
function union(a: Rect, b: Rect): Rect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return {
    x,
    y,
    w: Math.max(a.x + a.w, b.x + b.w) - x,
    h: Math.max(a.y + a.h, b.y + b.h) - y,
  };
}

/**
 * The scene of one spread.
 *
 * The emission order is the emitter's: photographs, then their captions,
 * then the text block, then the chapter caption. A caller that wants the
 * rectangles of a template nobody has chosen yet — the picker previewing a
 * candidate — wants `album.ts::slotsFor` instead: that question is about a
 * template, this one is about a spread.
 *
 * A template the catalogue does not know still renders, as one margined box,
 * silently: `slotsFor` falls back and so does this.
 */
export function sceneOf(
  spread: Spread,
  g: SpreadGeometry,
  measure: Measure,
): Scene {
  const rects = slotsFor(spread.template, spread.slots.length, g);
  const objects: SceneObject[] = [];

  spread.slots.forEach((slot, cell) => {
    const rect = rects[cell];
    if (!rect) return;
    objects.push({
      rect,
      angle: 0,
      reading: cell,
      role: {
        role: "photo",
        cell,
        src: slot.src,
        focal: slot.focal,
        zoom: slot.zoom ?? 1,
      },
    });
  });

  // The reading rank of everything below continues past the photographs: a
  // caption is read after the picture it names, and what belongs to the
  // whole spread comes last.
  let reading = spread.slots.length;

  spread.slots.forEach((slot, cell) => {
    const rect = rects[cell];
    if (!rect || !slot.caption) return;
    const at = { x: rect.x, y: rect.y + rect.h + PHOTO_CAPTION_DROP_MM };
    objects.push({
      rect: inkBox(at.x, at.y, slot.caption, PHOTO_CAPTION_SIZE_MM, measure),
      angle: 0,
      reading,
      role: { role: "photo_caption", cell, text: slot.caption, at },
    });
    reading += 1;
  });

  if (spread.text !== undefined && spread.text !== null) {
    const block = textBlock(spread, spread.text, g, measure);
    if (block) {
      objects.push({ ...block, reading });
      reading += 1;
    }
  }

  if (spread.caption !== undefined && spread.caption !== null) {
    const anchor = captionAnchor(spread.template, spread.slots.length, g);
    const at = { x: anchor.x, y: anchor.y };
    objects.push({
      rect: inkBox(at.x, at.y, spread.caption, CAPTION_SIZE_MM, measure),
      angle: 0,
      reading,
      role: { role: "chapter_caption", text: spread.caption, at },
    });
    reading += 1;
  }

  // The free objects come last, so they are on top: the order is the depth
  // here as in the engine, and what the reader placed covers what the
  // template produced.
  (spread.objets ?? []).forEach((objet, index) => {
    objects.push(objetLibre(index, objet, g, reading, measure));
    reading += 1;
  });

  return { objects };
}

/** The one text block a spread may carry, whichever of the three pages it
 *  is. Nothing set means no object: a focusable stop over a blank is worse
 *  than no stop at all. */
function textBlock(
  spread: Spread,
  text: string,
  g: SpreadGeometry,
  measure: Measure,
): SceneObject | null {
  let at: Point;
  let lines: SceneLine[];

  if (spread.template === GARDE_TEMPLATE) {
    at = gardeAnchor(g);
    lines = gardeLayout(text, gardePlace(g), measure).map((l) => ({
      text: l.texte,
      sizeMm: l.tailleMm,
      dyMm: l.dyMm,
      dxMm: 0,
    }));
  } else {
    const colophon = spread.template === COLOPHON_TEMPLATE;
    at = colophon ? colophonAnchor(g) : textAnchor(g);
    const sizeMm = colophon ? COLOPHON_SIZE_MM : TEXT_SIZE_MM;
    const leading = colophon ? COLOPHON_LEADING_MM : TEXT_LEADING_MM;
    // An empty line prints nothing and still takes its turn: the blank line
    // of a stored text is spacing, and the index is what spaces it.
    lines = text
      .split("\n")
      .map((l, i): SceneLine => ({ text: l, sizeMm, dyMm: i * leading, dxMm: 0 }))
      .filter((l) => l.text !== "");
  }

  if (lines.length === 0) return null;
  const rect = lines
    .map((l) => inkBox(at.x, at.y + l.dyMm, l.text, l.sizeMm, measure))
    .reduce(union);
  return { rect, angle: 0, reading: 0, role: { role: "text", at, lines } };
}

/**
 * One free object, laid out inside the box the reader drew.
 *
 * **This is the one place the two frames meet.** The object comes straight
 * out of `album.json`, so its box is in the engine's own frame — millimetres,
 * origin bottom-left — and everything else on this side is top-left. The flip
 * happens here, once, and reads like the flip the parity test performs on the
 * fixture, because it is the same flip.
 *
 * The angle is not flipped: it is kept as the engine stores it, and a
 * renderer negates it once through `angleEcran`.
 */
function objetLibre(
  index: number,
  objet: Objet,
  g: SpreadGeometry,
  reading: number,
  measure: Measure,
): SceneObject {
  const rect: Rect = { x: objet.x, y: g.h - (objet.y + objet.h), w: objet.w, h: objet.h };
  const tailleMm = objet.taille_pt * PT_MM;
  const interligne = interligneDe(objet);
  const align: Alignement = objet.alignement ?? "gauche";

  const { lignes, tropLarge } = replier(objet.texte, objet.w, tailleMm, measure);
  const lines: SceneLine[] = lignes.map((text, i) => ({
    text,
    sizeMm: tailleMm,
    dyMm: i * interligne,
    dxMm: decalage(align, objet.w, measure(text, tailleMm)),
  }));

  // The set height: the drop to the last baseline plus the line box that
  // baseline carries. Taller than the box means the text runs past the bottom
  // — which it may, out loud.
  const hauteur = (lines[lines.length - 1]?.dyMm ?? 0) + tailleMm * 1.35;
  const at: Point = { x: rect.x, y: rect.y + tailleMm };

  return {
    rect,
    angle: objet.angle ?? 0,
    reading,
    role: {
      role: "free_text",
      index,
      at,
      lines,
      align,
      overflow: hauteur > objet.h + 1e-9,
      tropLarge,
    },
  };
}

/**
 * The same scene with one cell framed differently — the gesture in flight,
 * before it lands on the undo stack.
 *
 * It belongs here rather than in a renderer because *both* renderers need
 * it, and because a draft is not a different scene: it is the same objects
 * with one framing not yet written down. Neither renderer has to know what
 * a crop draft is; they draw whatever scene they are handed.
 */
export function avecRecadrage(
  scene: Scene,
  cell: number,
  focal: [number, number],
  zoom: number,
): Scene {
  return {
    objects: scene.objects.map((o) =>
      o.role.role === "photo" && o.role.cell === cell
        ? { ...o, role: { ...o.role, focal, zoom } }
        : o,
    ),
  };
}

/** Whether a rectangle holds a point, edges included. Which object wins on
 *  a shared edge is not this function's problem: the paint order decides,
 *  and `hitTest` reads it from the front. */
export function contains(r: Rect, x: number, y: number): boolean {
  return x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h;
}

/**
 * What sits under a point, as the object's depth — or null for bare paper.
 *
 * The scene comes out in paint order, so the answer is the *last* object
 * that holds the point: a caption laid over a photograph is what the reader
 * sees there, and what a click must reach. This is the function that
 * replaces the `<div>` a canvas takes away, which is why it is pure, why it
 * is tested without a canvas, and why it was written before anything needed
 * it.
 */
export function hitTest(scene: Scene, x: number, y: number): number | null {
  for (let i = scene.objects.length - 1; i >= 0; i -= 1) {
    if (touche(scene.objects[i], x, y)) return i;
  }
  return null;
}

/** Whether an object holds a point, its angle included.
 *
 * A turned box is not tested with a turned test: the *point* is turned back
 * instead, by the object's own angle, and then the box is the upright box it
 * always was. One rotation instead of four half-plane tests, and the upright
 * case costs nothing because `tourner` is never called for it. */
export function touche(o: SceneObject, x: number, y: number): boolean {
  if (o.angle === 0) return contains(o.rect, x, y);
  const p = tourner({ x, y }, centre(o.rect), -o.angle);
  return contains(o.rect, p.x, p.y);
}

/** The depth of the photograph in a given cell, or null when that cell holds
 *  none. The editor still speaks in cells — a selection, a swap, a drop are
 *  all about the cell — and this is the one place that translation lives. */
export function depthOfCell(scene: Scene, cell: number): number | null {
  const at = scene.objects.findIndex(
    (o) => o.role.role === "photo" && o.role.cell === cell,
  );
  return at === -1 ? null : at;
}
