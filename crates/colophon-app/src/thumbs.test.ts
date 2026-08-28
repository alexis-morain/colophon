// The « sombre » badge, once a réglage stands between the photograph and the
// print. Two things are held here: that the corrected mean is the mean of the
// corrected photograph and not the correction of its mean, and that the badge
// itself reads that number rather than the raw one.
//
// The first is pure arithmetic on a histogram. The second has to go through
// `badgesDe`, because that is where the mistake would be made, and `badgesDe`
// samples a canvas — so this file lends it one. Vitest runs in node here: no
// DOM, no canvas package, and a stub of twenty lines is cheaper than either.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DARK_MEAN_LUMA, Rect, Reglage } from "./album";
import { badgesDe } from "./photos";
import { transfert } from "./reglage";
import { moyenneCorrigee, resetThumbs } from "./thumbs";

const IDENTITE: Reglage = { expo: 0, contraste: 0, nb: false };

/** A histogram from (luminance, count) pairs. */
function histo(...cases: [number, number][]): Uint32Array {
  const out = new Uint32Array(256);
  for (const [k, n] of cases) out[k] = n;
  return out;
}

/** The mean of a histogram: the corrected mean under the identity, which is
 *  the transfer that transfers nothing. */
const moyenne = (h: Uint32Array) => moyenneCorrigee(h, IDENTITE);

describe("la moyenne corrigée", () => {
  it("est la moyenne du transfert, jamais le transfert de la moyenne", () => {
    // Bimodal, and both ends clip once lifted: the shadows land on 0, the
    // highlights on 1. Between its clamps the transfer is affine and would
    // commute with the mean — the clipping is what makes the shortcut wrong,
    // and clipping is exactly what rescuing a photograph does.
    const h = histo([26, 1], [230, 1]);
    const r: Reglage = { expo: 0.5, contraste: 0.5, nb: false };

    const parLaDistribution = moyenneCorrigee(h, r);
    const parLaMoyenne = transfert(moyenne(h) / 255, r.expo, r.contraste) * 255;

    expect(parLaDistribution).toBeCloseTo(127.5, 3);
    expect(parLaMoyenne).toBeCloseTo(203.2, 1);
    expect(Math.abs(parLaDistribution - parLaMoyenne)).toBeGreaterThan(1);
  });

  it("rend la moyenne brute quand le réglage ne règle rien", () => {
    const h = histo([10, 3], [200, 1]);
    expect(moyenneCorrigee(h, IDENTITE)).toBeCloseTo((10 * 3 + 200) / 4, 9);
  });

  it("ne bouge pas sous le noir et blanc seul, sur un gris exact", () => {
    // Greying mixes the three channels into the luminance they average, so
    // on an already-grey photograph it preserves the mean exactly. The
    // transfer of `nb` alone is the identity, and the badge must not move.
    const gris = histo([40, 100], [90, 60], [150, 36]);
    expect(moyenneCorrigee(gris, { expo: 0, contraste: 0, nb: true })).toBe(
      moyenne(gris),
    );
  });

  it("est vide sur un histogramme vide plutôt que NaN", () => {
    expect(moyenneCorrigee(new Uint32Array(256), IDENTITE)).toBe(0);
  });
});

// ---- the badge itself ----------------------------------------------------

const TAILLE = 64;

/** Lends `thumbs.ts` a canvas that returns one flat grey, so a test can put a
 *  photograph of a known luminance in front of the badge. */
function poserCanvas(luma: number): void {
  const data = new Uint8ClampedArray(TAILLE * TAILLE * 4);
  for (let i = 0; i < data.length; i += 4) {
    data[i] = luma;
    data[i + 1] = luma;
    data[i + 2] = luma;
    data[i + 3] = 255;
  }
  (globalThis as any).document = {
    createElement: () => ({
      width: 0,
      height: 0,
      getContext: () => ({
        drawImage: () => {},
        getImageData: () => ({ data }),
      }),
    }),
  };
}

/** Enough of an `<img>` for the badges: they read three properties. */
const image = () =>
  ({ complete: true, naturalWidth: 800, naturalHeight: 600 }) as HTMLImageElement;

/** A cell far above the resolution floor, so only darkness is under test. */
const CASE: Rect = { x: 0, y: 0, w: 40, h: 30 };

const badgeSombre = (src: string, r?: Reglage) =>
  badgesDe(src, image(), CASE, 1, 1, r).dark;

describe("le badge « sombre »", () => {
  beforeEach(() => resetThumbs());
  afterEach(() => {
    delete (globalThis as any).document;
  });

  it("tombe quand l'exposition porte la photographie au-dessus du seuil", () => {
    poserCanvas(50); // under DARK_MEAN_LUMA: a night shot
    expect(badgeSombre("nuit.jpg")).toBe(true);

    // +0,5 EV: 50 becomes about 70, over the threshold. The photograph will
    // print light enough, so the badge has nothing left to warn about.
    const rattrapee: Reglage = { expo: 0.5, contraste: 0, nb: false };
    expect(moyenneCorrigee(histo([50, 1]), rattrapee)).toBeGreaterThan(
      DARK_MEAN_LUMA,
    );
    expect(badgeSombre("nuit.jpg", rattrapee)).toBe(false);

    // « Rendre à l'original » drops the réglage, and the badge comes back
    // from the same cache, without a reload.
    expect(badgeSombre("nuit.jpg")).toBe(true);
    expect(badgeSombre("nuit.jpg", IDENTITE)).toBe(true);
  });

  it("ne se laisse pas rattraper par un réglage qui n'en fait pas assez", () => {
    poserCanvas(30);
    expect(badgeSombre("cave.jpg", { expo: 0.5, contraste: 0, nb: false })).toBe(
      true,
    );
  });

  it("ne bouge pas quand la photographie est claire", () => {
    poserCanvas(180);
    expect(badgeSombre("jour.jpg")).toBe(false);
    expect(badgeSombre("jour.jpg", { expo: -0.2, contraste: 0, nb: false })).toBe(
      false,
    );
  });
});
