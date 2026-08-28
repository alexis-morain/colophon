// Port of `reglage.rs`: what an adjustment does to a pixel, and the CSS
// filter chain that says the same thing to the compositor.
//
// The transform is the CSS filter formula on purpose — grey is luma 709
// (`grayscale(1)`), exposure is `brightness(2^expo)`, contrast is
// `contrast(2^contraste)` around the 0,5 pivot, clamp between primitives —
// so the DOM shows the print's pixels for one line of style, and the canvas
// falls back to the LUT below where `ctx.filter` does not exist. The parity
// test holds this file to the engine's committed LUT dump, byte for byte.
//
// **If this diverges, it is the port that is wrong.** The engine renders the
// PDF; nothing here is allowed to be a second opinion about it.

import type { Reglage } from "./album";

/** Hard bounds of both sliders: enough to rescue a shot, not a darkroom. */
export const REGLAGE_BORNE = 1;

const clamp = (v: number, lo: number, hi: number) =>
  Math.min(Math.max(v, lo), hi);

/** The adjustment that adjusts nothing. Never stored. */
export function estIdentite(r: Reglage): boolean {
  return r.expo === 0 && r.contraste === 0 && !r.nb;
}

/** The stored adjustment of a photo, identity when there is none. */
export function reglageOuIdentite(r: Reglage | undefined): Reglage {
  return r ?? { expo: 0, contraste: 0, nb: false };
}

/** The mono-channel transfer on [0,1], clamped after each step like CSS
 *  clamps between primitives. Mirror of `reglage.rs::transfert`.
 *
 *  Exported for `thumbs.ts`, which needs the transfer of one luminance and
 *  not a table of 256: the « sombre » badge averages a histogram *through*
 *  this function. Not a second definition — the same one, read from one more
 *  place. */
export function transfert(v: number, expo: number, contraste: number): number {
  const b = Math.pow(2, clamp(expo, -REGLAGE_BORNE, REGLAGE_BORNE));
  const monte = clamp(v * b, 0, 1);
  const c = Math.pow(2, clamp(contraste, -REGLAGE_BORNE, REGLAGE_BORNE));
  return clamp((monte - 0.5) * c + 0.5, 0, 1);
}

/**
 * The 256-entry table of one adjustment's exposure and contrast, computed in
 * float as one block and rounded once — composing two u8 tables would
 * quantise twice and drift from the float-working CSS filter. `nb` is not in
 * it: grey mixes the three channels per pixel, before this table.
 */
export function lutDe(r: Reglage): number[] {
  const out = new Array<number>(256);
  for (let k = 0; k < 256; k++) {
    out[k] = Math.round(transfert(k / 255, r.expo, r.contraste) * 255);
  }
  return out;
}

/**
 * The CSS filter chain of one adjustment, `undefined` for the identity so
 * the style attribute stays clean. Order is the definition's — grey, then
 * exposure, then contrast — and every `<img>` surface reads this one
 * function, so the seven surfaces cannot disagree.
 */
export function filtreCss(r: Reglage | undefined): string | undefined {
  if (!r || estIdentite(r)) return undefined;
  const parts: string[] = [];
  if (r.nb) parts.push("grayscale(1)");
  if (r.expo !== 0) {
    parts.push(`brightness(${Math.pow(2, clamp(r.expo, -REGLAGE_BORNE, REGLAGE_BORNE))})`);
  }
  if (r.contraste !== 0) {
    parts.push(
      `contrast(${Math.pow(2, clamp(r.contraste, -REGLAGE_BORNE, REGLAGE_BORNE))})`,
    );
  }
  return parts.length > 0 ? parts.join(" ") : undefined;
}

/** Luma 709, the exact coefficients of CSS `grayscale(1)`. */
const LUMA_R = 0.2126;
const LUMA_G = 0.7152;
const LUMA_B = 0.0722;

/**
 * Apply one adjustment to decoded RGBA pixels, in place: the canvas
 * renderer's fallback where `ctx.filter` does not exist (WKWebView before
 * Safari 18). Colour channels are on the u8 grid, so the LUT is exact;
 * black and white mixes in float and runs the transfer directly, because a
 * grey quantised before the transfer would be the intermediate rounding the
 * whole module refuses. Mirror of `reglage.rs::appliquer`; alpha untouched.
 */
export function appliquer(pixels: Uint8ClampedArray, r: Reglage): void {
  if (r.nb) {
    for (let i = 0; i < pixels.length; i += 4) {
      const gris =
        (LUMA_R * pixels[i] + LUMA_G * pixels[i + 1] + LUMA_B * pixels[i + 2]) /
        255;
      const v = Math.round(transfert(clamp(gris, 0, 1), r.expo, r.contraste) * 255);
      pixels[i] = v;
      pixels[i + 1] = v;
      pixels[i + 2] = v;
    }
  } else {
    const table = lutDe(r);
    for (let i = 0; i < pixels.length; i += 4) {
      pixels[i] = table[pixels[i]];
      pixels[i + 1] = table[pixels[i + 1]];
      pixels[i + 2] = table[pixels[i + 2]];
    }
  }
}
