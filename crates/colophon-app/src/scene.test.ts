// The scene's own arithmetic, without a canvas and without the engine.
//
// The exhaustive comparison lives in `parity.test.ts`: nine spreads, six
// page shapes, against the engine's golden dump. What is tested here is what
// the fixture cannot show — the hit test that replaces the `<div>` a canvas
// takes away, and the two rules the assembler applies on top of the dump.

import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import { Album, Spread, spreadGeometry } from "./album";
import { Dump, setGeometrie } from "./geometrie";
import { contains, depthOfCell, hitTest, sceneOf } from "./scene";

setGeometrie(fixture as unknown as Dump);

const g = spreadGeometry({
  trim_mm: fixture.trim_mm,
  bleed_mm: fixture.bleed_mm,
} as Album);

/** A width proportional to the glyph count: enough to give captions ink,
 *  and no font file in sight. */
const mesure = (s: string, tailleMm: number) => s.length * tailleMm * 0.5;

function planche(template: string, n: number): Spread {
  return {
    template,
    slots: Array.from({ length: n }, (_, i) => ({
      src: `${i}.jpg`,
      focal: [0.5, 0.42] as [number, number],
    })),
  };
}

describe("what sits under a point", () => {
  it("answers nothing for bare paper", () => {
    const scene = sceneOf(planche("duo_portrait", 2), g, mesure);
    // The gutter runs down the middle of a two-cell spread and belongs to
    // no object: the click that lands there deselects, it does not select.
    expect(hitTest(scene, g.w / 2, g.h / 2)).toBeNull();
    // And nothing at all lies outside the media box.
    expect(hitTest(scene, -10, -10)).toBeNull();
  });

  it("answers the photograph under the pointer", () => {
    const scene = sceneOf(planche("duo_portrait", 2), g, mesure);
    const first = scene.objects[0].rect;
    const at = hitTest(scene, first.x + first.w / 2, first.y + first.h / 2);
    expect(at).toBe(0);
    expect(scene.objects[at!].role).toMatchObject({ role: "photo", cell: 0 });
  });

  it("reads from the front: a caption laid over a photograph wins", () => {
    const spread = planche("full1", 1);
    spread.slots[0].caption = "la traversée";
    const scene = sceneOf(spread, g, mesure);
    // A full-bleed photograph covers the whole media box, so its caption —
    // painted after it — sits over it. The reader sees the caption there,
    // and so must the hit test.
    const legende = scene.objects[1];
    expect(legende.role.role).toBe("photo_caption");
    const at = hitTest(scene, legende.rect.x + 1, legende.rect.y + 1);
    expect(at).toBe(1);
  });

  it("is the paint order read backwards, whatever the reading order says", () => {
    const spread = planche("quad", 4);
    spread.caption = "un chapitre";
    const scene = sceneOf(spread, g, mesure);
    // The chapter caption is last painted and last read here; what the hit
    // test uses is the depth, and the depth is the index.
    const dernier = scene.objects.length - 1;
    const r = scene.objects[dernier].rect;
    expect(hitTest(scene, r.x + 0.5, r.y + 0.5)).toBe(dernier);
  });

  it("holds a point on its own edge", () => {
    const r = { x: 10, y: 20, w: 30, h: 40 };
    expect(contains(r, 10, 20)).toBe(true);
    expect(contains(r, 40, 60)).toBe(true);
    expect(contains(r, 40.001, 60)).toBe(false);
  });
});

describe("cells and depths", () => {
  it("translates a cell into a depth, and says so when there is none", () => {
    const scene = sceneOf(planche("quad", 4), g, mesure);
    expect(depthOfCell(scene, 2)).toBe(2);
    expect(depthOfCell(scene, 9)).toBeNull();
  });

  it("keeps the reading rank of a caption behind its own photograph", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "la plage";
    const scene = sceneOf(spread, g, mesure);
    expect(scene.objects.map((o) => o.role.role)).toEqual([
      "photo",
      "photo",
      "photo_caption",
    ]);
    expect(scene.objects.map((o) => o.reading)).toEqual([0, 1, 2]);
  });
});

describe("what the assembler adds on top of the dump", () => {
  it("gives an empty caption no object at all", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "";
    expect(sceneOf(spread, g, mesure).objects).toHaveLength(2);
  });

  it("renders a template nobody knows as one margined box, silently", () => {
    const scene = sceneOf(planche("gabarit-repare-a-la-main", 3), g, mesure);
    expect(scene.objects).toHaveLength(1);
    expect(scene.objects[0].role.role).toBe("photo");
  });

  it("makes the three text pages one role", () => {
    for (const template of ["garde", "texte", "colophon"]) {
      const spread = planche(template, 0);
      spread.text = "Un été\n\nCalvi, Corse";
      const scene = sceneOf(spread, g, mesure);
      expect(scene.objects).toHaveLength(1);
      const role = scene.objects[0].role;
      expect(role.role).toBe("text");
      // The blank line prints nothing and still takes its turn.
      if (role.role === "text") {
        expect(role.lines.map((l) => l.text)).toEqual(["Un été", "Calvi, Corse"]);
        expect(role.lines[1].dyMm).toBeGreaterThan(0);
      }
    }
  });

  it("gives a text page with nothing written no object", () => {
    const spread = planche("texte", 0);
    spread.text = "";
    expect(sceneOf(spread, g, mesure).objects).toHaveLength(0);
  });
});
