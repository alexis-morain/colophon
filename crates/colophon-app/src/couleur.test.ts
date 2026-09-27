// La couleur d'un objet libre, côté écran : la grammaire du moteur, ses deux
// défauts, et les six pastilles du popover.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { Objet } from "./album";
import {
  couleurDe,
  couleurValide,
  ENCRE_LIVRE,
  ENCRE_ORNEMENT,
  PASTILLES,
  TEXT_INK,
  versHex,
} from "./couleur";

/** Une constante `[f64; 3]` de `pdf.rs`, lue dans le source : le miroir se
 *  vérifie contre l'original, pas contre une copie tenue à la main. */
function constanteDuMoteur(nom: string): number[] {
  const source = readFileSync(
    fileURLToPath(new URL("../../colophon-core/src/pdf.rs", import.meta.url)),
    "utf8",
  );
  const m = source.match(new RegExp(`pub const ${nom}: \\[f64; 3\\] = \\[([^\\]]+)\\]`));
  if (!m) throw new Error(`${nom} introuvable dans pdf.rs`);
  return m[1].split(",").map((v) => Number(v.trim()));
}

describe("versHex", () => {
  it("rend l'encre de texte du livre en #rrggbb, arrondie comme le moteur la relit", () => {
    // 0,2 × 255 = 51 ; 0,19 × 255 = 48,45 ; 0,16 × 255 = 40,8.
    expect(versHex(TEXT_INK)).toBe("#333029");
    expect(ENCRE_LIVRE).toBe("#333029");
  });

  it("borne et complète chaque composante sur deux chiffres", () => {
    expect(versHex([0, 0, 0])).toBe("#000000");
    expect(versHex([1, 1, 1])).toBe("#ffffff");
    expect(versHex([1.2, -0.1, 0.02])).toBe("#ff0005");
  });
});

describe("les encres par défaut sont celles du moteur", () => {
  it("TEXT_INK", () => {
    expect([...TEXT_INK]).toEqual(constanteDuMoteur("TEXT_INK"));
  });

  it("ORNEMENT_INK", () => {
    expect(ENCRE_ORNEMENT).toBe(versHex(constanteDuMoteur("ORNEMENT_INK") as [number, number, number]));
  });
});

describe("couleurValide", () => {
  it.each(["#000000", "#b04a1f", "#B04A1F"])("accepte %s", (c) => {
    expect(couleurValide(c)).toBe(true);
  });

  it.each(["#abc", "rouge", "#12345g", "", "b04a1f", "#b04a1f0"])("refuse %s", (c) => {
    expect(couleurValide(c)).toBe(false);
  });
});

describe("couleurDe", () => {
  const bloc: Objet = { x: 0, y: 0, w: 40, h: 10, type: "texte", texte: "a", taille_pt: 12 };
  const orn: Objet = { x: 0, y: 0, w: 40, h: 4, type: "ornement", pack: "colophon", id: "f" };

  it("rend la couleur posée", () => {
    expect(couleurDe({ ...bloc, couleur: "#b04a1f" })).toBe("#b04a1f");
    expect(couleurDe({ ...orn, couleur: "#999999" })).toBe("#999999");
  });

  it("retombe sur l'encre du livre pour un bloc, le noir pour un ornement", () => {
    expect(couleurDe(bloc)).toBe(ENCRE_LIVRE);
    expect(couleurDe(orn)).toBe("#000000");
  });

  it("retombe sur le défaut quand la chaîne est hors grammaire, comme le PDF", () => {
    expect(couleurDe({ ...bloc, couleur: "rouge" })).toBe(ENCRE_LIVRE);
    expect(couleurDe({ ...orn, couleur: "#abc" })).toBe("#000000");
  });
});

describe("les pastilles", () => {
  it("sont six, l'encre du livre d'abord, qui retire la couleur", () => {
    expect(PASTILLES).toHaveLength(6);
    expect(PASTILLES[0]).toEqual({ cle: "livre", hex: null });
    expect(PASTILLES.map((p) => p.hex)).toEqual([
      null,
      "#000000",
      "#ffffff",
      "#999999",
      "#4d4d4d",
      "#b04a1f",
    ]);
  });
});
