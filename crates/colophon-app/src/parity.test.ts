// Geometry parity without a running dev server: execute the engine's own
// dump and diff it against the TypeScript port, for every page shape.
// Skipped when the release binary is missing; scripts/check.sh builds it
// first, so CI never skips.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import sceneFixture from "./scene.fixture.json";
import { geometryProblems, PARITY_FORMATS } from "./parity";

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
  // Nothing reads this yet: the renderer that will is wave 2.3's. It is
  // committed now so that the port lands on a model already pinned, and so
  // that any change to what a spread *means* shows up as a diff here rather
  // than as a surprise on screen.
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
