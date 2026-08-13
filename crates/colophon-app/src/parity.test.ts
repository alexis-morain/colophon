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
