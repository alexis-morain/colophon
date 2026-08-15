// Geometry parity without a running dev server: execute the engine's own
// dump and diff it against the TypeScript port, for every page shape.
// Skipped when the release binary is missing; scripts/check.sh builds it
// first, so CI never skips.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { geometryProblems, PARITY_FORMATS } from "./parity";

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
