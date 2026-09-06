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

/**
 * Un ornement se retaille en gardant le rapport de son dessin.
 *
 * Ce n'est pas une question de goût : sa boîte **est** son encre, et c'est
 * cette égalité qui rend honnête tout ce qui ne mesure jamais que le rectangle
 * — le pli, la coupe, le prévol, les deux compteurs du linter. Un fleuron
 * étiré est laid ; un fleuron dont la boîte ment au prévol est un livre gâché.
 */
describe("tailler, à rapport imposé", () => {
  // Le rapport de `filet-fleche` : 12 sur 17. Mesuré à l'écran le 06/09 après
  // un glissement de coin, la boîte rendait 0,7058.
  const R = 12 / 17;

  it("garde le rapport, quelle que soit la direction du geste", () => {
    for (const [dx, dy] of [
      [40, 0],
      [0, 40],
      [40, 40],
      [-20, 5],
      [60, -30],
    ]) {
      const a = tailler({ rect: { x: 40, y: 60, w: 12, h: 17 }, angle: 0 }, 2, dx, dy, R);
      expect(a.rect.w / a.rect.h).toBeCloseTo(R, 9);
    }
  });

  it("couvre ce que la main demande, dans les deux axes", () => {
    // La boîte suit le pointeur plutôt que d'en moyenner les deux courses :
    // un geste qui reculerait sous la main serait un geste qu'on ne peut pas
    // viser.
    const a = tailler({ rect: { x: 0, y: 0, w: 12, h: 17 }, angle: 0 }, 2, 30, 0, R);
    expect(a.rect.w).toBeCloseTo(42, 9);
    const b = tailler({ rect: { x: 0, y: 0, w: 12, h: 17 }, angle: 0 }, 2, 0, 30, R);
    expect(b.rect.h).toBeCloseTo(47, 9);
    expect(b.rect.w).toBeCloseTo(47 * R, 9);
  });

  it("garde le coin opposé fixe, tourné comme droit", () => {
    for (const angle of [0, 30, -46, 137]) {
      const avant: PoseObjet = { rect: { x: 40, y: 60, w: 12, h: 17 }, angle };
      const apres = tailler(avant, 1, 9, -11, R);
      const fixe = corners(avant.rect, angle)[FIXE[1]];
      const encore = corners(apres.rect, apres.angle)[FIXE[1]];
      expect(encore.x).toBeCloseTo(fixe.x, 6);
      expect(encore.y).toBeCloseTo(fixe.y, 6);
    }
  });

  it("respecte le plancher sans casser le rapport", () => {
    // Le plancher entre dans le même maximum que le rapport : l'appliquer
    // après écraserait la forme au moment précis où la boîte devient petite.
    const a = tailler({ rect: { x: 0, y: 0, w: 12, h: 17 }, angle: 0 }, 2, -200, -200, R);
    expect(a.rect.w).toBeGreaterThanOrEqual(4);
    expect(a.rect.h).toBeGreaterThanOrEqual(4);
    expect(a.rect.w / a.rect.h).toBeCloseTo(R, 9);
  });
});
