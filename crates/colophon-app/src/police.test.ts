// Ce que le sélecteur montre, et ce qu'il refuse de montrer.

import { describe, expect, it } from "vitest";
import { EN, FR, setLangue } from "./i18n";
import { REFUS_KEYS, nomLisible, parFamille, refusLibelle } from "./police";

describe("le nom lisible d'une face", () => {
  it("élague le « Regular » que le moteur colle au nom", () => {
    // La dette nommée le 28/08 : le moteur colle famille (ID 1) et style
    // (ID 2), et il doit continuer — ce que le fichier déclare est ce que le
    // fichier déclare. C'est l'écran qui décide de ne pas le lire à voix
    // haute.
    expect(nomLisible({ nom: "MuktaMahee Medium Regular" })).toBe("MuktaMahee Medium");
    expect(nomLisible({ nom: "Helvetica Neue Regular" })).toBe("Helvetica Neue");
    // Un style qui n'est pas « Regular » reste entier.
    expect(nomLisible({ nom: "Helvetica Neue Bold" })).toBe("Helvetica Neue Bold");
    expect(nomLisible({ nom: "Optima Italic" })).toBe("Optima Italic");
    // Et on n'élague jamais jusqu'au vide : une face qui ne s'appelle que
    // « Regular » garde son nom plutôt que de devenir une ligne blanche.
    expect(nomLisible({ nom: "Regular" })).toBe("Regular");
    expect(nomLisible({ nom: "", famille: "Optima" })).toBe("Optima");
  });
});

describe("les faces groupées par famille", () => {
  const face = (rang: number, famille: string, nom: string, refus: string | null = null) => ({
    rang,
    famille,
    nom,
    postscript: nom.replace(/\s/g, ""),
    refus,
  });

  it("range chaque face sous sa famille, refusées comprises", () => {
    // Une face refusée reste dans la liste : la cacher enverrait quelqu'un
    // chercher une police qui est bien là.
    const g = parFamille([
      face(0, "Optima", "Optima Regular"),
      face(1, "Helvetica Neue", "Helvetica Neue Bold"),
      face(2, "Optima", "Optima Bold", "embarquement_interdit"),
    ]);
    expect(g.map((f) => f.famille)).toEqual(["Helvetica Neue", "Optima"]);
    expect(g[1].faces).toHaveLength(2);
    expect(g[1].faces[1].refus).toBe("embarquement_interdit");
  });

  it("range les faces internes de la plate-forme après les autres", () => {
    // macOS en porte des centaines, toutes nommées par un point initial, et
    // un point trie avant une lettre : sans la règle, les quarante premières
    // familles de la liste sont toutes internes et les vraies n'apparaissent
    // qu'au filtre. Elles restent montrées, elles passent après.
    const g = parFamille([
      face(0, ".SF NS", ".SF NS Regular"),
      face(1, "Optima", "Optima Regular"),
      face(2, ".ADT Slab Numeric", ".ADT Slab Numeric Light"),
      face(3, "Avenir", "Avenir Book"),
    ]);
    expect(g.map((f) => f.famille)).toEqual([
      "Avenir",
      "Optima",
      ".ADT Slab Numeric",
      ".SF NS",
    ]);
  });

  it("laisse tomber la face que rien ne nomme", () => {

    // Le seul refus qui coûte son nom à une face est `illisible` : elle n'a
    // ni famille ni nom ni PostScript, donc aucune ligne où se ranger.
    expect(parFamille([{ rang: 0, famille: "", nom: "", postscript: "", refus: "illisible" }]))
      .toHaveLength(0);
  });
});

describe("les raisons d'un refus", () => {
  it("sont dites dans les deux langues, et jamais par leur code", () => {
    for (const code of REFUS_KEYS) {
      const cle = `police.refus.${code}`;
      expect(FR).toHaveProperty(cle);
      expect(EN).toHaveProperty(cle);
    }
    setLangue("fr");
    expect(refusLibelle("embarquement_interdit")).toBe(
      "sa licence interdit de l’incorporer",
    );
    setLangue("en");
    expect(refusLibelle("embarquement_interdit")).toBe(
      "its licence forbids embedding",
    );
    // Un code que l'app ne connaît pas se montre plutôt que de disparaître.
    expect(refusLibelle("un_refus_de_demain")).toBe("un_refus_de_demain");
  });
});
