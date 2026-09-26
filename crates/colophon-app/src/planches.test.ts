// Le glisser des planches au pointeur : la cible est un calcul pur, testé
// ici sans DOM. La couverture est la cellule zéro de la grille, et elle ne
// reçoit jamais une planche.

import { describe, expect, it } from "vitest";
import { cibleSous, RectCellule, seuilFranchi, SEUIL_GLISSER_PX } from "./planches";

const cellule = (at: number, left: number, top: number): RectCellule => ({
  at,
  left,
  top,
  right: left + 100,
  bottom: top + 80,
});

// Une rangée : la couverture, puis trois planches, 12 px de gouttière.
const RANGEE = [
  cellule(-1, 0, 0),
  cellule(0, 112, 0),
  cellule(1, 224, 0),
  cellule(2, 336, 0),
];

describe("cibleSous", () => {
  it("rend la planche dont le rectangle contient le pointeur", () => {
    expect(cibleSous(RANGEE, 150, 40)).toBe(0);
    expect(cibleSous(RANGEE, 400, 10)).toBe(2);
  });

  it("compte les bords comme dedans", () => {
    expect(cibleSous(RANGEE, 224, 0)).toBe(1);
    expect(cibleSous(RANGEE, 324, 80)).toBe(1);
  });

  it("ne rend rien hors de toute cellule", () => {
    expect(cibleSous(RANGEE, 600, 40)).toBeNull();
    expect(cibleSous(RANGEE, 150, 200)).toBeNull();
  });

  it("ne rend rien entre deux cellules, dans la gouttière", () => {
    expect(cibleSous(RANGEE, 218, 40)).toBeNull();
  });

  it("n'offre jamais la couverture comme cible", () => {
    expect(cibleSous(RANGEE, 50, 40)).toBeNull();
  });

  it("ne rend rien sur une grille vide", () => {
    expect(cibleSous([], 10, 10)).toBeNull();
  });
});

describe("seuilFranchi", () => {
  it("laisse un clic rester un clic sous le seuil", () => {
    expect(seuilFranchi(0, 0)).toBe(false);
    expect(seuilFranchi(3, 4)).toBe(false);
    expect(seuilFranchi(SEUIL_GLISSER_PX - 0.1, 0)).toBe(false);
  });

  it("commence un glisser au seuil, dans tous les sens", () => {
    expect(seuilFranchi(SEUIL_GLISSER_PX, 0)).toBe(true);
    expect(seuilFranchi(0, -SEUIL_GLISSER_PX)).toBe(true);
    expect(seuilFranchi(-5, 5)).toBe(true);
  });
});
