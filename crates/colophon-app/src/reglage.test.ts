// The adjustment port, held to the engine's committed LUT dump, byte for
// byte — same contract as the geometry and the scene fixtures: the unit
// tests run on the committed fixture, and the freshness test refuses a
// fixture the engine no longer produces. Changing a luma coefficient or a
// clamp on one side only makes the parity fall.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import fixture from "./reglage.fixture.json";
import { appliquer, estIdentite, filtreCss, lutDe } from "./reglage";

const BINARY = fileURLToPath(
  new URL("../../../target/release/colophon", import.meta.url),
);

type Entree = { expo: number; contraste: number; lut: number[] };

describe("LUT parity with the engine", () => {
  it("the port matches the committed dump, byte for byte", () => {
    expect(fixture.grille.length).toBeGreaterThan(0);
    for (const entree of fixture.grille as Entree[]) {
      expect(
        lutDe({ expo: entree.expo, contraste: entree.contraste, nb: false }),
        `expo ${entree.expo}, contraste ${entree.contraste}`,
      ).toEqual(entree.lut);
    }
  });

  it("the identity LUT is entry k = k", () => {
    const table = lutDe({ expo: 0, contraste: 0, nb: false });
    for (let k = 0; k < 256; k++) expect(table[k]).toBe(k);
  });
});

describe.skipIf(!existsSync(BINARY))("LUT fixture freshness", () => {
  it("the committed fixture is the engine's current dump", () => {
    const fresh = JSON.parse(
      execFileSync(BINARY, ["--dump-lut"], { encoding: "utf8" }),
    );
    expect(fixture).toEqual(fresh);
  });
});

describe("the CSS filter chain", () => {
  it("says nothing for the identity", () => {
    expect(filtreCss({ expo: 0, contraste: 0, nb: false })).toBeUndefined();
    expect(filtreCss(undefined)).toBeUndefined();
  });

  it("keeps the definition's order: grey, exposure, contrast", () => {
    expect(filtreCss({ expo: 1, contraste: -1, nb: true })).toBe(
      "grayscale(1) brightness(2) contrast(0.5)",
    );
    // A partial adjustment only names what it changes.
    expect(filtreCss({ expo: 0.5, contraste: 0, nb: false })).toBe(
      `brightness(${Math.pow(2, 0.5)})`,
    );
    expect(filtreCss({ expo: 0, contraste: 0, nb: true })).toBe("grayscale(1)");
  });
});

describe("the pixel fallback (canvas without ctx.filter)", () => {
  it("colour goes through the LUT, alpha untouched", () => {
    const px = new Uint8ClampedArray([128, 64, 200, 255]);
    appliquer(px, { expo: 1, contraste: 0, nb: false });
    const table = lutDe({ expo: 1, contraste: 0, nb: false });
    expect([...px]).toEqual([table[128], table[64], table[200], 255]);
  });

  it("black and white mixes luma 709 in float, channels equal", () => {
    // Green 100 under +0,5 stop: 101 through the float chain, 102 through a
    // grey quantised before the transfer — the value that tells them apart,
    // same witness as the Rust test.
    const px = new Uint8ClampedArray([0, 100, 0, 255]);
    appliquer(px, { expo: 0.5, contraste: 0, nb: true });
    expect([...px]).toEqual([101, 101, 101, 255]);
  });

  it("identity means identity", () => {
    expect(estIdentite({ expo: 0, contraste: 0, nb: false })).toBe(true);
    expect(estIdentite({ expo: 0, contraste: 0, nb: true })).toBe(false);
    expect(estIdentite({ expo: 0.1, contraste: 0, nb: false })).toBe(false);
  });
});
