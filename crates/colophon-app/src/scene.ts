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
  captionAnchor,
  colophonAnchor,
  COLOPHON_LEADING_MM,
  COLOPHON_SIZE_MM,
  COLOPHON_TEMPLATE,
  CAPTION_SIZE_MM,
  gardeAnchor,
  gardeLayout,
  gardePlace,
  GARDE_TEMPLATE,
  PHOTO_CAPTION_DROP_MM,
  PHOTO_CAPTION_SIZE_MM,
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
 *  the block's first baseline. */
export type SceneLine = { text: string; sizeMm: number; dyMm: number };

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
  | { role: "text"; at: Point; lines: SceneLine[] };

/** One visible element: where it is, when it is read, and what it is. No
 *  rotation, no matrix — an oriented box arrives with the free objects of
 *  wave 6, and with its own linter counter. */
export type SceneObject = { rect: Rect; reading: number; role: Role };

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
      reading,
      role: { role: "chapter_caption", text: spread.caption, at },
    });
  }

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
      .map((l, i): SceneLine => ({ text: l, sizeMm, dyMm: i * leading }))
      .filter((l) => l.text !== "");
  }

  if (lines.length === 0) return null;
  const rect = lines
    .map((l) => inkBox(at.x, at.y + l.dyMm, l.text, l.sizeMm, measure))
    .reduce(union);
  return { rect, reading: 0, role: { role: "text", at, lines } };
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
    if (contains(scene.objects[i].rect, x, y)) return i;
  }
  return null;
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
