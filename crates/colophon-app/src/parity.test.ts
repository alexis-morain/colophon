// Geometry parity without a running dev server: execute the engine's own
// dump and diff it against the TypeScript port, for every page shape.
// Skipped when the release binary is missing; scripts/check.sh builds it
// first, so CI never skips.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import sceneAlbum from "./scene.album.json";
import sceneFixture from "./scene.fixture.json";
import { Album, spreadGeometry } from "./album";
import { setGeometrie } from "./geometrie";
import { geometryProblems, PARITY_FORMATS, sceneProblems } from "./parity";

/// The page shapes the raster gate walks, and the scene fixture with them.
const SCENE_FORMATS = [
  "carre-21",
  "carre-30",
  "portrait-a4",
  "paysage-a4",
  "paysage-28x21",
  "portrait-20x25",
];

const BINARY = fileURLToPath(
  new URL("../../../target/release/colophon", import.meta.url),
);

describe.skipIf(!existsSync(BINARY))("geometry parity with the engine", () => {
  it.each(PARITY_FORMATS)("format %s matches the PDF geometry", (format) => {
    const dump = JSON.parse(
      execFileSync(BINARY, ["--dump-geometry", "--format", format], {
        encoding: "utf8",
      }),
    );
    expect(geometryProblems(dump, format)).toEqual([]);
  });

  // The unit tests run on a committed fixture of this dump: stale fixture,
  // stale tests, so the engine's current output must be the fixture.
  it("the committed fixture is the engine's current dump", () => {
    const fresh = JSON.parse(
      execFileSync(BINARY, ["--dump-geometry", "--format", "carre-21"], {
        encoding: "utf8",
      }),
    );
    expect(fixture).toEqual(fresh);
  });

  // The scene of a committed album, on every page shape. Its input is
  // `scene.album.json`, hand-written to hold one of everything — the
  // half-title, a full-bleed page, an empty caption that must produce no
  // object, a truncated verso, a declared caption band, a text page with a
  // blank line, a template nobody knows, the colophon — because a fixture
  // taken from a real album covers whatever that album happened to contain.
  //
  // Two tests hang off it: this one keeps it honest against the engine, and
  // the one below holds the TypeScript port to it.
  it.each(SCENE_FORMATS)("the committed scene fixture holds for %s", (format) => {
    const fresh = JSON.parse(
      execFileSync(
        BINARY,
        ["--dump-scene", fileURLToPath(new URL("./scene.album.json", import.meta.url)),
         "--format", format],
        { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
      ),
    );
    expect((sceneFixture as Record<string, unknown>)[format]).toEqual(fresh);
  });
});

// The scene port, held to the same fixture. `scene.ts` assembles what a
// spread holds so the renderer can draw it without asking the engine — and
// an assembler that agrees with the engine on nine spreads' worth of order,
// roles, reading ranks and line breaks, on six page shapes, is an assembler
// whose renderer is drawing the book that will print.
//
// **If this diverges, it is the port that is wrong.** The engine writes the
// PDF; nothing here is allowed to be a second opinion about it.
describe.skipIf(!existsSync(BINARY))("scene parity with the engine", () => {
  it.each(SCENE_FORMATS)("the port assembles %s's scenes", (format) => {
    const dump = JSON.parse(
      execFileSync(BINARY, ["--dump-geometry", "--format", format], {
        encoding: "utf8",
      }),
    );
    setGeometrie(dump);
    const g = spreadGeometry({
      trim_mm: dump.trim_mm,
      bleed_mm: dump.bleed_mm,
    } as Album);
    expect(
      sceneProblems(
        (sceneFixture as Record<string, unknown>)[format],
        (sceneAlbum as Album).spreads,
        g,
        format,
      ),
    ).toEqual([]);
  });
});

// A photo that exactly fills its cell has nothing to slide, and the editor has
// to say so rather than swallow the gesture. This is the arithmetic behind
// that sentence; the drag and the tooltip both read it, so it is tested once.
describe("sliding room inside a cell", () => {
  it("is nil when the photo and the cell share a shape", async () => {
    const { slidingRoom } = await import("./album");
    // 4:3 in a 4:3 cell: the cover-crop fits exactly, both ways.
    const r = slidingRoom({ w: 400, h: 300 }, 1600, 1200);
    expect(r.x).toBeCloseTo(0, 9);
    expect(r.y).toBeCloseTo(0, 9);
  });

  it("hangs over the long side when the shapes disagree", async () => {
    const { slidingRoom } = await import("./album");
    // A 2:1 panorama in a square cell: it covers by height, and 200 px of
    // width hang over — 100 on each side of the framing.
    const r = slidingRoom({ w: 200, h: 200 }, 2000, 1000);
    expect(r.x).toBeCloseTo(200, 9);
    expect(r.y).toBeCloseTo(0, 9);
  });

  it("is what the zoom buys back", async () => {
    const { slidingRoom } = await import("./album");
    const exact = slidingRoom({ w: 400, h: 300 }, 1600, 1200, 2);
    expect(exact.x).toBeCloseTo(400, 9);
    expect(exact.y).toBeCloseTo(300, 9);
    // Below the fill, zoom never flatters: it clamps to 1, like the print.
    expect(slidingRoom({ w: 400, h: 300 }, 1600, 1200, 0.5).x).toBeCloseTo(0, 9);
  });

  it("answers nothing rather than NaN for an image of no size", async () => {
    const { slidingRoom } = await import("./album");
    expect(slidingRoom({ w: 400, h: 300 }, 0, 0)).toEqual({ x: 0, y: 0 });
  });
});

// The badge maths is a port too: print.rs::print_scale via prevol.rs.
describe("effective print resolution (port of print_scale)", () => {
  it("reads the cover-crop scale, and zoom crops further in", async () => {
    const { effectivePpi } = await import("./album");
    // 1000 px across a 100 mm cell: 0,1 mm per pixel, 254 ppi exactly.
    expect(effectivePpi({ w: 100, h: 50 }, 1000, 1000)).toBeCloseTo(254, 5);
    // The tighter side rules: same photo, taller cell, half the resolution.
    expect(effectivePpi({ w: 100, h: 200 }, 1000, 1000)).toBeCloseTo(127, 5);
    // Manual zoom eats pixels linearly; below 1 it never flatters.
    expect(effectivePpi({ w: 100, h: 50 }, 1000, 1000, 2)).toBeCloseTo(127, 5);
    expect(effectivePpi({ w: 100, h: 50 }, 1000, 1000, 0.5)).toBeCloseTo(254, 5);
  });
});
