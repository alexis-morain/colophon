// Le dump du format, et la fenêtre blanche.
//
// Une bascule change le rognage de l'album sous un éditeur déjà rendu. Le
// dump chargé à l'ouverture décrit l'ancienne page ; celui de la nouvelle
// n'existe nulle part, et `geometrie()` jette plutôt que de rendre un
// rectangle faux — ce qui est le bon choix, sauf qu'aucune frontière
// d'erreur ne couvre l'arbre React : la levée démonte tout, et il ne reste
// qu'une fenêtre blanche.
//
// Ce fichier tient les deux moitiés du remède : le dump de la cible se
// charge **avant** que le bilan s'affiche, et `adopterGeometrie` refuse
// quand il manque, ce qui empêche d'appliquer un album que personne ne
// saurait dessiner.

import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import {
  adopterGeometrie,
  Dump,
  geometrie,
  geometrieCourante,
  setGeometrie,
  setGeometrieFormat,
} from "./geometrie";

const carre = fixture as unknown as Dump;
/** Le même dump à une autre page : ce test ne mesure pas des rectangles,
 *  il mesure quel dump répond à quelle demande. */
const paysage: Dump = {
  ...carre,
  trim_mm: { w: 297, h: 210 },
  media: { ...carre.media, w: 600, h: 216 },
};

describe("la géométrie d'un album qui change de format", () => {
  it("ne connaît pas le format d'arrivée tant que personne ne l'a chargé", () => {
    setGeometrie(carre);
    // La levée d'origine, celle qui vidait la fenêtre.
    expect(() => geometrie(paysage.trim_mm, paysage.bleed_mm)).toThrow();
    // Et la garde qui l'empêche d'arriver à l'écran : le panneau refuse
    // d'appliquer plutôt que d'appliquer et de disparaître.
    expect(adopterGeometrie(paysage.trim_mm, paysage.bleed_mm)).toBe(false);
    expect(geometrieCourante().trim_mm).toEqual(carre.trim_mm);
  });

  it("dessine le format d'arrivée dès que son dump est là", () => {
    setGeometrie(carre);
    setGeometrieFormat(paysage);
    expect(geometrie(paysage.trim_mm, paysage.bleed_mm).media.w).toBe(600);
    expect(adopterGeometrie(paysage.trim_mm, paysage.bleed_mm)).toBe(true);
    expect(geometrieCourante().trim_mm).toEqual(paysage.trim_mm);
  });

  it("garde l'ancien format, qui est ce que ⌘Z ramène", () => {
    setGeometrie(carre);
    setGeometrieFormat(paysage);
    adopterGeometrie(paysage.trim_mm, paysage.bleed_mm);
    // Annuler la bascule remet l'album carré sous un dump paysage : les
    // rectangles doivent se retrouver quand même, sans quoi ⌘Z ferait
    // exactement ce que la bascule faisait.
    expect(geometrie(carre.trim_mm, carre.bleed_mm).media.w).toBe(
      carre.media.w,
    );
    expect(adopterGeometrie(carre.trim_mm, carre.bleed_mm)).toBe(true);
    expect(geometrieCourante().trim_mm).toEqual(carre.trim_mm);
  });

  it("distingue deux fonds perdus sous le même rognage", () => {
    // L'écran de création charge ses aperçus à fond perdu nul, un album
    // vit à trois millimètres : confondre les deux rendrait à la bascule un
    // dump dont chaque page déborde de trois millimètres.
    setGeometrie(carre);
    expect(adopterGeometrie(carre.trim_mm, 0)).toBe(false);
    setGeometrieFormat({ ...carre, bleed_mm: 0 });
    expect(adopterGeometrie(carre.trim_mm, 0)).toBe(true);
    expect(geometrieCourante().bleed_mm).toBe(0);
  });
});
