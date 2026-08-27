// Ce que `focal` veut dire, tenu du côté TypeScript pour lui-même. Le test de
// parité dit que ce port égale le moteur ; il ne dit pas que l'un des deux a
// raison. Ces assertions-là disent laquelle des deux lectures est la bonne.

import { describe, expect, it } from "vitest";
import { cropWindow, imageSpan, slidingRoom } from "./album";

const IW = 4000;
const IH = 3000;

describe("cropWindow", () => {
  // Tout 3.1 tient dans ce test : un `focal` est un point de la photo, donc
  // une bascule de format ne déplace pas ce que l'œil a cadré. Zoom 2, où les
  // deux axes ont du jeu et où rien n'est donc borné.
  it("centre la fenêtre sur le même point quel que soit le ratio", () => {
    const focal: [number, number] = [0.62, 0.38];
    for (const rect of [
      { w: 300, h: 200 },
      { w: 200, h: 300 },
    ]) {
      const [x0, y0, vw, vh] = cropWindow(rect, IW, IH, focal, 2);
      expect(x0 + vw / 2).toBeCloseTo(focal[0] * IW, 6);
      expect(y0 + vh / 2).toBeCloseTo(focal[1] * IH, 6);
    }
  });

  it("ancre au bord sans sortir de l'image", () => {
    for (const focal of [
      [0, 0],
      [1, 1],
    ] as [number, number][]) {
      const [x0, y0, vw, vh] = cropWindow({ w: 300, h: 200 }, IW, IH, focal, 2);
      expect(x0).toBeGreaterThanOrEqual(0);
      expect(y0).toBeGreaterThanOrEqual(0);
      expect(x0 + vw).toBeLessThanOrEqual(IW + 1e-9);
      expect(y0 + vh).toBeLessThanOrEqual(IH + 1e-9);
    }
  });
});

describe("imageSpan", () => {
  // Le jeu est l'empan moins la cellule : une seule arithmétique, deux
  // questions. Si les deux se mettent à diverger, c'est ici que ça se voit.
  it("le jeu est l'empan moins la cellule", () => {
    const rect = { w: 300, h: 200 };
    for (const zoom of [1, 1.7, 4]) {
      const span = imageSpan(rect, IW, IH, zoom);
      const room = slidingRoom(rect, IW, IH, zoom);
      expect(room.x).toBeCloseTo(span.x - rect.w, 9);
      expect(room.y).toBeCloseTo(span.y - rect.h, 9);
    }
  });

  // Le glissement divise par l'empan : à zoom 1, une photo au ratio de sa
  // cellule occupe exactement la cellule, donc un pixel de doigt vaut un
  // pixel d'image — et il n'y a aucun jeu pour le dépenser.
  it("rend zéro sur une image vide plutôt que NaN", () => {
    expect(imageSpan({ w: 300, h: 200 }, 0, 0)).toEqual({ x: 0, y: 0 });
  });
});
