// Le pack d'ornements côté écran : ce que l'app en fait, et ce qu'elle refuse
// d'en inventer.
//
// Deux registres, et la distinction compte. Les tests unitaires travaillent
// sur un pack posé à la main, parce qu'ils portent sur la traduction en tracé
// et pas sur le contenu du pack livré. Le dernier `describe` lance le moteur :
// c'est le seul endroit qui peut dire que ce que l'app reçoit ressemble
// vraiment à ce que le moteur envoie, et il se saute sans le binaire, comme
// `parity.test.ts`.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { setLangue } from "./i18n";
import {
  attributD,
  attributViewBox,
  Ornement,
  ornementDe,
  ornements,
  rapport,
  setOrnements,
  titre,
} from "./ornement";

const CARRE: Ornement = {
  id: "carre",
  famille: "filet",
  titre_fr: "Le carré",
  titre_en: "The square",
  licence: "CC0-1.0",
  auteur: "personne",
  source: "https://example.invalid/",
  dessin: {
    viewbox: [0, 0, 20, 10],
    chemins: [
      {
        evenodd: false,
        segments: [
          { op: "vers", x: 0, y: 0 },
          { op: "ligne", x: 20, y: 0 },
          { op: "courbe", x1: 20, y1: 5, x2: 10, y2: 10, x: 0, y: 10 },
          { op: "ferme" },
        ],
      },
    ],
  },
};

describe("attributD", () => {
  it("écrit les quatre opérateurs et rien d'autre", () => {
    expect(attributD(CARRE.dessin.chemins[0])).toBe(
      "M0 0 L20 0 C20 5 10 10 0 10 Z",
    );
  });

  // Le dessin arrive normalisé du moteur : absolu, droites et cubiques. Si
  // un `H`, un `V` ou une commande relative apparaissait ici, c'est que
  // `ornement.rs` aurait cessé de replier, et les deux rendus se mettraient à
  // interpréter du SVG chacun de son côté.
  it("ne produit jamais une commande que le moteur ne replie pas", () => {
    const d = attributD(CARRE.dessin.chemins[0]);
    expect(d).not.toMatch(/[HhVvSsQqTtAa]/);
    expect(d).toMatch(/^M/);
  });

  it("rend le viewBox tel quel", () => {
    expect(attributViewBox(CARRE.dessin)).toBe("0 0 20 10");
  });
});

describe("rapport", () => {
  // C'est lui que la poignée de redimensionnement lit : la boîte d'un
  // ornement garde le rapport de son dessin, donc sa boîte EST son encre, et
  // le pli comme la coupe mesurent exactement ce qui s'imprime.
  it("est celui du viewBox", () => {
    expect(rapport(CARRE.dessin)).toBe(2);
  });
});

describe("le pack", () => {
  it("rend undefined pour un identifiant qu'il ne porte pas", () => {
    setOrnements([CARRE]);
    expect(ornementDe("carre")).toBe(CARRE);
    // Un `album.json` se répare à la main : un identifiant inconnu est un
    // état atteignable, pas une erreur. Le moteur ne dessine rien et
    // n'échoue pas ; l'écran fait pareil.
    expect(ornementDe("inexistant")).toBeUndefined();
  });

  it("est vide tant que rien ne l'a posé, et ne jette pas", () => {
    setOrnements([]);
    expect(ornements()).toEqual([]);
    expect(ornementDe("carre")).toBeUndefined();
  });

  it("dit son titre dans la langue de l'écran", () => {
    setOrnements([CARRE]);
    setLangue("fr");
    expect(titre(CARRE)).toBe("Le carré");
    setLangue("en");
    expect(titre(CARRE)).toBe("The square");
    setLangue("fr");
  });
});

const BINARY = fileURLToPath(
  new URL("../../../target/release/colophon", import.meta.url),
);

describe.skipIf(!existsSync(BINARY))("le pack du moteur", () => {
  const pack: Ornement[] = JSON.parse(
    execFileSync(BINARY, ["--dump-ornements"], { encoding: "utf8" }),
  );

  it("arrive dans la forme que ce module suppose", () => {
    expect(pack.length).toBeGreaterThan(0);
    for (const o of pack) {
      expect(["fleuron", "filet", "separateur"]).toContain(o.famille);
      // La liste blanche vit dans le moteur ; on vérifie ici qu'elle n'a pas
      // fui vers l'écran par une entrée mal formée.
      expect(["CC0-1.0", "PD"]).toContain(o.licence);
      expect(o.auteur).not.toBe("");
      expect(o.source).not.toBe("");
      expect(o.titre_fr).not.toBe("");
      expect(o.titre_en).not.toBe("");
      const [, , w, h] = o.dessin.viewbox;
      expect(w).toBeGreaterThan(0);
      expect(h).toBeGreaterThan(0);
      expect(o.dessin.chemins.length).toBeGreaterThan(0);
      for (const c of o.dessin.chemins) {
        // Tout chemin commence par un déplacement : c'est ce que le moteur
        // garantit, et ce dont `Path2D` a besoin pour ne pas partir de zéro.
        expect(c.segments[0].op).toBe("vers");
        expect(attributD(c)).not.toMatch(/[HhVvSsQqTtAa]/);
      }
    }
  });
});
