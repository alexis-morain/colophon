// Où s'ouvre un popover ancré à un objet de la planche : sous lui, au-dessus
// quand le bas de la fenêtre est proche, et jamais hors de la fenêtre.

import { describe, expect, it } from "vitest";
import { enveloppe, largeurLegende, placerSous } from "./popover";

const FENETRE = { w: 900, h: 620 };

describe("largeurLegende", () => {
  it("prend la largeur de la case quand elle tient", () => {
    expect(largeurLegende(300)).toBe(300);
  });

  it("ne dépasse jamais 420 px", () => {
    expect(largeurLegende(700)).toBe(420);
  });

  it("ne descend jamais sous 240 px", () => {
    expect(largeurLegende(90)).toBe(240);
  });
});

describe("placerSous", () => {
  const ancre = { left: 100, top: 100, right: 400, bottom: 300 };

  it("s'ouvre sous l'objet, aligné sur son bord gauche", () => {
    expect(placerSous(ancre, { w: 300, h: 46 }, FENETRE)).toEqual({
      left: 100,
      top: 308,
    });
  });

  it("passe au-dessus quand le bas de la fenêtre est proche", () => {
    const bas = { left: 100, top: 400, right: 400, bottom: 590 };
    expect(placerSous(bas, { w: 300, h: 46 }, FENETRE)).toEqual({
      left: 100,
      top: 400 - 46 - 8,
    });
  });

  it("recule pour tenir dans la largeur de la fenêtre", () => {
    const droite = { left: 700, top: 100, right: 880, bottom: 300 };
    expect(placerSous(droite, { w: 420, h: 46 }, FENETRE).left).toBe(900 - 420 - 8);
  });

  it("ne sort jamais par la gauche", () => {
    const gauche = { left: -40, top: 100, right: 60, bottom: 300 };
    expect(placerSous(gauche, { w: 240, h: 46 }, FENETRE).left).toBe(8);
  });

  it("ne sort jamais par le haut quand il passe au-dessus", () => {
    const haute = { left: 100, top: 20, right: 400, bottom: 600 };
    expect(placerSous(haute, { w: 300, h: 80 }, FENETRE).top).toBe(8);
  });
});

describe("enveloppe", () => {
  it("rend le rectangle droit qui contient tous les points", () => {
    expect(
      enveloppe([
        { x: 10, y: 0 },
        { x: 20, y: 10 },
        { x: 10, y: 20 },
        { x: 0, y: 10 },
      ]),
    ).toEqual({ left: 0, top: 0, right: 20, bottom: 20 });
  });
});
