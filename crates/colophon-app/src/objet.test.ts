// Le redimensionnement d'un objet libre, sans fenêtre.
//
// C'est la seule arithmétique du calque qui ne se lit pas d'un coup d'œil : on
// retaille une boîte tournée le long de ses propres bords, en gardant fixe le
// coin opposé. L'invariant qui compte tient en une phrase — **le coin opposé
// ne bouge pas dans le monde** — et il se teste à tous les angles.

import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import { Dump, setGeometrie } from "./geometrie";

setGeometrie(fixture as unknown as Dump);

import { PoseObjet, tailler } from "./ObjetLibreCalque";
import { corners } from "./scene";

/** Le coin que chaque poignée laisse fixe, dans l'ordre du calque. */
const FIXE = [2, 3, 0, 1];

const pose = (angle: number): PoseObjet => ({
  rect: { x: 40, y: 60, w: 80, h: 30 },
  angle,
});

describe("tailler", () => {
  it("suit la main sur une boîte droite, coin opposé fixe", () => {
    // Coin bas-droite (indice 2 dans l'ordre du calque), tiré de 10 × 6.
    const apres = tailler(pose(0), 2, 10, 6);
    expect(apres.rect.w).toBeCloseTo(90, 9);
    expect(apres.rect.h).toBeCloseTo(36, 9);
    // Le haut-gauche n'a pas bougé.
    expect(apres.rect.x).toBeCloseTo(40, 9);
    expect(apres.rect.y).toBeCloseTo(60, 9);
  });

  it("tire aussi par le haut-gauche, et c'est le bas-droite qui tient", () => {
    const apres = tailler(pose(0), 0, -10, -6);
    expect(apres.rect.w).toBeCloseTo(90, 9);
    expect(apres.rect.h).toBeCloseTo(36, 9);
    expect(apres.rect.x + apres.rect.w).toBeCloseTo(120, 9);
    expect(apres.rect.y + apres.rect.h).toBeCloseTo(90, 9);
  });

  it("laisse le coin opposé exactement où il était, à tous les angles", () => {
    // L'invariant du geste. S'il tombe, la boîte glisse sous la main pendant
    // qu'on la retaille — le défaut le plus désagréable d'une poignée.
    for (const angle of [0, 17.5, 45, 90, -30, 179]) {
      const avant = pose(angle);
      const avantCoins = corners(avant.rect, angle);
      for (let coin = 0; coin < 4; coin += 1) {
        const apres = tailler(avant, coin, 12, -7);
        const apresCoins = corners(apres.rect, apres.angle);
        const f = FIXE[coin];
        expect(apresCoins[f].x).toBeCloseTo(avantCoins[f].x, 6);
        expect(apresCoins[f].y).toBeCloseTo(avantCoins[f].y, 6);
      }
    }
  });

  it("garde l'angle : une poignée de coin ne tourne rien", () => {
    expect(tailler(pose(33), 2, 5, 5).angle).toBe(33);
  });

  it("retaille le long des bords de la boîte, pas de ceux de l'écran", () => {
    // À 90°, tirer vers la droite de l'écran allonge la boîte selon *sa*
    // hauteur, pas selon sa largeur. C'est ce qu'une main attend d'une
    // poignée de coin, et c'est ce qui distingue ce calcul d'une soustraction.
    const droite = tailler(pose(0), 2, 20, 0);
    expect(droite.rect.w).toBeCloseTo(100, 9);
    expect(droite.rect.h).toBeCloseTo(30, 9);

    const tourne = tailler(pose(90), 2, 20, 0);
    expect(tourne.rect.w).toBeCloseTo(80, 6);
    expect(tourne.rect.h).toBeCloseTo(50, 6);
  });

  it("ne laisse pas une boîte devenir insaisissable", () => {
    const ecrase = tailler(pose(0), 2, -200, -200);
    expect(ecrase.rect.w).toBeGreaterThanOrEqual(4);
    expect(ecrase.rect.h).toBeGreaterThanOrEqual(4);
  });
});
