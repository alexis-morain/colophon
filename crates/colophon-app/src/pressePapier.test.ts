// Le presse-papier d'un objet libre, sans fenêtre.
//
// Ce qui entre par un collage vient du système, donc de n'importe où : une
// autre fenêtre, une autre version, un autre programme qui aurait posé le même
// type. `lire` est la seule porte, et elle refuse tout ce qui ne ferait pas un
// objet que l'album sait dessiner.

import { describe, expect, it } from "vitest";
import { Objet } from "./album";
import { TYPE_OBJET, lire, serialiser } from "./pressePapier";

/** Un `DataTransfer` réduit à ce que `lire` en consulte. */
const donnees = (types: Record<string, string>) => ({
  getData: (type: string) => types[type] ?? "",
});

const bloc: Objet = {
  x: 20,
  y: 30,
  w: 80,
  h: 16,
  angle: 12,
  type: "texte",
  texte: "Corse, été 2013",
  taille_pt: 12,
  interligne_mm: 6,
  alignement: "centre",
};

const ornement: Objet = {
  x: 50,
  y: 60,
  w: 40,
  h: 4,
  type: "ornement",
  pack: "colophon",
  id: "filet-simple",
};

/** Le JSON d'un objet, abîmé d'un champ. */
const abime = (champs: Record<string, unknown>) =>
  donnees({ [TYPE_OBJET]: JSON.stringify({ ...bloc, ...champs }) });

describe("serialiser puis lire", () => {
  it("rend le bloc tel qu'il était, avec la planche d'où il vient", () => {
    const colle = lire(donnees(serialiser(bloc, 3)));
    expect(colle).toEqual({ ...bloc, de: 3 });
  });

  it("rend l'ornement, sans texte à coller ailleurs", () => {
    const pose = serialiser(ornement, 0);
    expect(pose["text/plain"]).toBe("");
    expect(lire(donnees(pose))).toEqual({ ...ornement, de: 0 });
  });

  it("pose le texte du bloc en texte brut, pour un autre programme", () => {
    expect(serialiser(bloc, 0)["text/plain"]).toBe("Corse, été 2013");
  });

  it("ne garde que les champs d'un objet, pas ce qu'on aurait glissé à côté", () => {
    const colle = lire(abime({ chemin: "/etc/passwd" }));
    expect(colle).not.toBeNull();
    expect(colle).not.toHaveProperty("chemin");
  });
});

describe("lire refuse", () => {
  it("un type inconnu", () => {
    expect(lire(abime({ type: "image" }))).toBeNull();
  });

  it("une boîte non finie ou sous 4 mm", () => {
    expect(lire(abime({ x: null }))).toBeNull();
    expect(lire(abime({ w: "80" }))).toBeNull();
    expect(lire(abime({ w: 1e400 }))).toBeNull();
    expect(lire(abime({ w: 3.9, h: 3.9 }))).toBeNull();
    expect(lire(abime({ h: 0 }))).toBeNull();
    expect(lire(abime({ h: -2 }))).toBeNull();
  });

  it("mais pas un filet mince, que l'app pose elle-même sous 4 mm de haut", () => {
    // Un filet de 20:1 naît au tiers d'une page de 195,5 mm : 3,26 mm de
    // haut. Le refuser rendrait incopiable ce que la pose vient de produire.
    const filet = { ...ornement, w: 65.17, h: 3.26 };
    expect(lire(donnees(serialiser(filet, 0)))).toEqual({ ...filet, de: 0 });
  });

  it("un angle non fini", () => {
    expect(lire(abime({ angle: "12" }))).toBeNull();
    expect(lire(abime({ angle: null }))).toBeNull();
  });

  it("un contenu absent ou un texte qui n'est pas une chaîne", () => {
    const { texte: _t, ...sansTexte } = bloc as Objet & { texte: string };
    expect(lire(donnees({ [TYPE_OBJET]: JSON.stringify(sansTexte) }))).toBeNull();
    expect(lire(abime({ texte: 42 }))).toBeNull();
    expect(lire(abime({ taille_pt: 0 }))).toBeNull();
  });

  it("un ornement d'un pack inconnu", () => {
    const etranger = { ...ornement, pack: "ailleurs" };
    expect(lire(donnees({ [TYPE_OBJET]: JSON.stringify(etranger) }))).toBeNull();
  });

  it("un JSON étranger, sans lever", () => {
    expect(lire(donnees({ [TYPE_OBJET]: "{pas du json" }))).toBeNull();
    expect(lire(donnees({ [TYPE_OBJET]: "[1, 2]" }))).toBeNull();
    expect(lire(donnees({ [TYPE_OBJET]: "null" }))).toBeNull();
  });

  it("un texte brut seul : rien à coller", () => {
    expect(lire(donnees({ "text/plain": "Corse, été 2013" }))).toBeNull();
  });
});
