// Ce que le sélecteur montre, et ce qu'il refuse de montrer.

import { describe, expect, it } from "vitest";
import { EN, FR, setLangue } from "./i18n";
import { PoliceOfferte } from "./bridge";
import {
  REFUS_KEYS,
  faceReguliere,
  faceVoisine,
  nomLisible,
  parFamille,
  refusLibelle,
  selection,
  voixDe,
} from "./police";

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
    gras: false,
    italique: false,
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
    expect(parFamille([{ rang: 0, famille: "", nom: "", postscript: "", refus: "illisible", gras: false, italique: false }]))
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

describe("les dix familles suggérées", () => {
  const face = (rang: number, famille: string, nom: string, refus: string | null = null) => ({
    rang,
    famille,
    nom,
    postscript: nom.replace(/\s/g, ""),
    refus,
    gras: false,
    italique: false,
  });

  // Une machine de série, en désordre, avec ce qu'une vraie porte autour :
  // des familles internes, des styles, et une face que sa licence refuse.
  const machine = [
    face(0, ".SF NS", ".SF NS Regular"),
    face(1, "Helvetica Neue", "Helvetica Neue Bold"),
    face(2, "Helvetica Neue", "Helvetica Neue Regular"),
    face(3, "Helvetica Neue", "Helvetica Neue Italic"),
    face(4, "Optima", "Optima Regular"),
    face(5, "Georgia", "Georgia Regular"),
    face(6, "Didot", "Didot Regular"),
    face(7, "Menlo", "Menlo Regular"),
    face(8, "Comic Sans MS", "Comic Sans MS Regular"),
    face(9, "Zapfino", "Zapfino Regular"),
  ];

  it("n'en rend jamais plus de dix", () => {
    // Le panneau montrait les 787 faces de la machine. Personne ne compose
    // un album en parcourant huit cents noms.
    expect(selection(machine).length).toBeLessThanOrEqual(10);
  });

  it("prend une famille par voix, dans l'ordre des voix", () => {
    // Une linéale, une humaniste, un romain de texte, une didone, une
    // chasse fixe : ce qui est offert est varié par construction, et non
    // « les dix premières de l'alphabet ».
    expect(selection(machine).map((p) => p.famille)).toEqual([
      "Helvetica Neue",
      "Optima",
      "Georgia",
      "Didot",
      "Menlo",
    ]);
  });

  it("ne suggère que ce que la machine porte", () => {
    // Cinq voix n'ont trouvé personne ici : compléter avec une deuxième
    // famille d'une voix déjà servie rendrait le nombre et perdrait la
    // variété, qui est tout ce que la liste promet.
    const noms = selection(machine).map((p) => p.famille);
    expect(noms).not.toContain("Comic Sans MS");
    expect(noms).not.toContain("Zapfino");
    expect(noms).not.toContain(".SF NS");
  });

  it("choisit la droite de la famille, pas son italique", () => {
    const helvetica = selection(machine)[0];
    expect(helvetica.nom).toBe("Helvetica Neue Regular");
  });

  it("passe à la famille suivante quand la première est refusée", () => {
    // Une face qu'un PDF ne peut pas porter n'est pas une suggestion :
    // elle reste dans la liste complète, avec sa raison, et la voix va
    // chercher la suivante qu'elle connaît.
    const refusee = [
      face(0, "Helvetica Neue", "Helvetica Neue Regular", "embarquement_interdit"),
      face(1, "Arial", "Arial Regular"),
    ];
    expect(selection(refusee).map((p) => p.famille)).toEqual(["Arial"]);
  });

  it("ne place jamais deux fois la même famille", () => {
    // Courier New porte deux voix, la machine à écrire et la chasse fixe :
    // la première servie la prend, la seconde va chercher ailleurs.
    const noms = selection([
      face(0, "Courier New", "Courier New Regular"),
      face(1, "Monaco", "Monaco Regular"),
    ]).map((p) => p.famille);
    expect(noms).toEqual(["Courier New", "Monaco"]);
  });

  it("dit de quelle voix une suggestion est", () => {
    setLangue("fr");
    expect(voixDe(face(0, "Didot", "Didot Regular"))).toBe("didone");
    expect(voixDe(face(0, "Menlo", "Menlo Regular"))).toBe("mono");
    // Une famille qu'aucune voix ne nomme n'invente pas de note.
    expect(voixDe(face(0, "Zapfino", "Zapfino Regular"))).toBeNull();
  });

  it("nomme chaque voix dans les deux langues", () => {
    for (const p of selection(machine)) {
      const cle = `police.voix.${voixDe(p)}`;
      expect(FR, cle).toHaveProperty(cle);
      expect(EN, cle).toHaveProperty(cle);
    }
  });
});

// A-s2 : gras et italique d'un bloc. Ce sont des faces de la famille, lues
// dans leurs tables (`gras`, `italique`), jamais une simulation.
describe("gras et italique dans une famille", () => {
  const face = (
    rang: number,
    nom: string,
    gras: boolean,
    italique: boolean,
    refus: string | null = null,
  ): PoliceOfferte => ({
    rang,
    famille: "Helvetica Neue",
    nom,
    postscript: nom.replace(/ /g, ""),
    refus,
    gras,
    italique,
  });
  const helvetica = [
    face(0, "Helvetica Neue", false, false),
    face(1, "Helvetica Neue Bold", true, false),
    face(2, "Helvetica Neue Italic", false, true),
    face(3, "Helvetica Neue Bold Italic", true, true),
    face(4, "Helvetica Neue Condensed Bold", true, false),
    face(5, "Helvetica Neue Light", false, false),
    face(6, "Helvetica Neue Light Italic", false, true),
  ];
  const [regular, bold, italic, boldItalic, , light, lightItalic] = helvetica;

  it("la face régulière est ni grasse ni italique", () => {
    expect(faceReguliere(helvetica)).toBe(regular);
    // Sinon la première non refusée.
    const toutesGrasses = [face(0, "X Black", true, false, "illisible"), face(1, "X Bold", true, false)];
    expect(faceReguliere(toutesGrasses)?.rang).toBe(1);
    expect(faceReguliere([face(0, "X", false, false, "illisible")])).toBeNull();
  });

  it("G passe de la droite au gras, et du gras à la droite", () => {
    expect(faceVoisine(helvetica, regular, "gras")).toBe(bold);
    expect(faceVoisine(helvetica, bold, "gras")).toBe(regular);
  });

  it("I passe de la droite à l'italique, et garde le gras", () => {
    expect(faceVoisine(helvetica, regular, "italique")).toBe(italic);
    expect(faceVoisine(helvetica, bold, "italique")).toBe(boldItalic);
    expect(faceVoisine(helvetica, boldItalic, "italique")).toBe(bold);
    expect(faceVoisine(helvetica, boldItalic, "gras")).toBe(italic);
  });

  it("garde la graisse d'une face maigre quand elle a sa voisine", () => {
    expect(faceVoisine(helvetica, light, "italique")).toBe(lightItalic);
    expect(faceVoisine(helvetica, lightItalic, "italique")).toBe(light);
  });

  it("ne trouve rien dans une famille sans italique", () => {
    const sans = [face(0, "Didot", false, false), face(1, "Didot Bold", true, false)];
    expect(faceVoisine(sans, sans[0], "italique")).toBeNull();
    expect(faceVoisine(sans, sans[1], "italique")).toBeNull();
    expect(faceVoisine(sans, sans[0], "gras")).toBe(sans[1]);
  });

  it("ne propose jamais une face refusée", () => {
    const refusee = [regular, face(1, "Helvetica Neue Bold", true, false, "embarquement_interdit")];
    expect(faceVoisine(refusee, regular, "gras")).toBeNull();
  });
});
