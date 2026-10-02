// Ce que la mesure de l'écran a le droit de nommer.
//
// Le piège de la session : écrire `font-family: "Helvetica Neue"` marche sur
// la machine qui a la police, et c'est exactement ce qui rend le défaut
// invisible — l'album se mesure bien ici, mal ailleurs, et le crénage de la
// face installée décale les mesures d'un cheveu même ici. Ce test est le
// mordant : il regarde la chaîne que `measureMm` pose vraiment sur le
// contexte, pas une constante à côté.
//
// La mesure elle-même — l'écran et le PDF qui tombent sur le même
// millimètre — se prouve dans un navigateur, faute de fonte sous Vitest :
// `scripts/police-cdp.mjs` en face du banc `banc_parite_ecran_papier`.

import { beforeAll, describe, expect, it } from "vitest";

/** Un contexte de canvas qui n'a d'autre travail que de retenir ce qu'on
 *  lui demande. Posé avant l'import du module, parce que `font.ts` crée son
 *  contexte au premier appel et le garde. */
const ctx = {
  font: "",
  fontKerning: "auto",
  measureText: (t: string) => ({ width: t.length * 50 }),
};

/** Les faces posées sur le document, pour voir ce que `font.ts` y ajoute
 *  et y retire. */
const posees = new Set<unknown>();

beforeAll(() => {
  (globalThis as unknown as { document: unknown }).document = {
    createElement: () => ({ getContext: () => ctx }),
    fonts: {
      load: () => Promise.resolve([]),
      add: (f: unknown) => posees.add(f),
      delete: (f: unknown) => posees.delete(f),
    },
  };
  // Une `FontFace` qui se charge toujours : ce qui échoue, dans ces tests,
  // c'est la lecture des octets, pas le navigateur.
  (globalThis as unknown as { FontFace: unknown }).FontFace = class {
    constructor(
      public family: string,
      public octets: ArrayBuffer,
      public options: Record<string, string>,
    ) {}
    load() {
      return Promise.resolve(this);
    }
  };
});

describe("la mesure de l'écran", () => {
  it("nomme la face de l'album, jamais une police installée", async () => {
    const { measureMm, FAMILLE } = await import("./font");
    measureMm("Corse, 2013", 10);

    expect(ctx.font.startsWith(`100px "${FAMILLE}"`)).toBe(true);
    expect(FAMILLE).toBe("colophon-album");
    // Aucune face du système ne se nomme ici. « Source Sans 3 » n'est
    // présente qu'en repli, derrière la face de l'album, et c'est celle que
    // le moteur embarque quand l'album n'a rien choisi — jamais celle d'une
    // machine.
    for (const installee of [
      "Helvetica",
      "Helvetica Neue",
      "Arial",
      "Times",
      "Optima",
      "system-ui",
      "-apple-system",
    ]) {
      expect(ctx.font).not.toContain(installee);
    }
    // Le dernier recours est générique et sans empattement : une légende
    // mesurée contre une serif de secours signale un débordement qui
    // n'existe pas, et c'est le piège que `fontLoaded()` existe pour fermer.
    const pile = ctx.font.replace(/^100px /, "").split(", ");
    expect(pile).toHaveLength(3);
    expect(pile[2]).toBe("sans-serif");

    expect(ctx.font.indexOf(FAMILLE)).toBeLessThan(
      ctx.font.indexOf("Source Sans 3"),
    );
  });

  it("coupe le crénage, que le moteur ne dessine pas", async () => {
    const { measureMm } = await import("./font");
    measureMm("A", 10);
    // Le moteur demande un glyphe par caractère et additionne les chasses.
    // Un navigateur crène par défaut : sans ça, l'écran mesurerait une ligne
    // que l'imprimeur ne composera jamais.
    expect(ctx.fontKerning).toBe("none");
  });

  it("mesure en grand et divise, pour que l'arrondi du navigateur ne compte pas", async () => {
    const { measureMm } = await import("./font");
    // La fixture rend 50 px par caractère à 100 px de corps : onze
    // caractères font 5,5 em, donc 5,5 fois la taille demandée.
    expect(measureMm("Corse, 2013", 10)).toBeCloseTo(55, 9);
    expect(measureMm("", 10)).toBe(0);
  });
});

/** L'écran dessine « ? » là où le moteur imprime « ? » : la même règle,
 *  caractère par caractère, sur le texte comme sur la mesure. */
describe("la substitution", () => {
  it("remplace exactement les caractères que le moteur a nommés", async () => {
    const { afficher, setAbsents, substituer, measureMm } = await import("./font");
    setAbsents([]);
    expect(afficher("Zażółć gęślą jaźń")).toBe("Zażółć gęślą jaźń");
    setAbsents(["ż", "ę"]);
    expect(afficher("Zażółć gęślą jaźń")).toBe("Za?ółć g?ślą jaźń");
    // La mesure suit : un « ? » de large là où l'autre police aurait prêté
    // un glyphe d'une autre chasse.
    expect(measureMm("ż", 10)).toBe(measureMm("?", 10));
    const scene = {
      objects: [
        { rect: { x: 0, y: 0, w: 1, h: 1 }, role: { role: "photo_caption", cell: 0, text: "gęślą", at: { x: 0, y: 0 } } },
        { rect: { x: 0, y: 0, w: 1, h: 1 }, role: { role: "text", at: { x: 0, y: 0 }, lines: [{ text: "jaźń ż", sizeMm: 3, dyMm: 0, dxMm: 0 }] } },
        { rect: { x: 0, y: 0, w: 1, h: 1 }, role: { role: "photo", cell: 0, src: "ż.jpg", focal: [0, 0], zoom: 1 } },
      ],
    };
    const vue = substituer(scene);
    expect((vue.objects[0].role as { text: string }).text).toBe("g?ślą");
    expect((vue.objects[1].role as { lines: { text: string }[] }).lines[0].text).toBe("jaźń ?");
    expect((vue.objects[2].role as { src: string }).src).toBe("ż.jpg");
    setAbsents([]);
    expect(substituer(scene)).toBe(scene);
  });
});

/** La face d'un bloc : une famille interne de plus, devant celle du livre. */
describe("la face d'un bloc", () => {
  const police = {
    fichier: "objet-HelveticaNeue-Bold.ttf",
    postscript: "HelveticaNeue-Bold",
    nom: "Helvetica Neue Bold",
  };

  it("se nomme d'après son fichier, jamais d'après la police installée", async () => {
    const { familleObjet } = await import("./font");
    expect(familleObjet(police)).toBe("colophon-objet-HelveticaNeue-Bold");
    expect(familleObjet({ ...police, fichier: "objet-Didot.otf" })).toBe(
      "colophon-objet-Didot",
    );
  });

  it("se mesure devant la face du livre, qui la remplace tant qu'elle manque", async () => {
    const { measureMm, FAMILLE } = await import("./font");
    measureMm("Corse", 10, "colophon-objet-HelveticaNeue-Bold");
    expect(ctx.font).toBe(
      `100px "colophon-objet-HelveticaNeue-Bold", "${FAMILLE}", "Source Sans 3", sans-serif`,
    );
    expect(ctx.font).not.toContain("Helvetica Neue");
    // Sans famille, la pile du livre, telle qu'avant.
    measureMm("Corse", 10);
    expect(ctx.font).toBe(`100px "${FAMILLE}", "Source Sans 3", sans-serif`);
  });

  it("tient la grammaire du moteur", async () => {
    const { fichierObjetValide } = await import("./font");
    expect(fichierObjetValide("objet-HelveticaNeue-Bold.ttf")).toBe(true);
    expect(fichierObjetValide("objet-Didot.otf")).toBe(true);
    expect(fichierObjetValide(`objet-${"a".repeat(64)}.ttf`)).toBe(true);
    for (const faux of [
      "police.ttf",
      "objet-.ttf",
      "../objet-a.ttf",
      "objet-a/b.ttf",
      "objet-a.woff",
      "objet-a_b.ttf",
      `objet-${"a".repeat(65)}.ttf`,
    ]) {
      expect(fichierObjetValide(faux)).toBe(false);
    }
  });

  it("se charge une fois par fichier, dit celles qui manquent, s'oublie au changement d'album", async () => {
    const { chargerFacesObjet, faceObjetManque, oublierFacesObjet, tourDeFace } =
      await import("./font");
    const lus: string[] = [];
    const lire = async (f: string) => {
      lus.push(f);
      if (f === "objet-Parti.ttf") throw new Error("fichier_absent");
      return new ArrayBuffer(8);
    };
    const avant = tourDeFace();
    await chargerFacesObjet(["objet-A.ttf", "objet-Parti.ttf", "objet-A.ttf"], lire);
    expect(lus).toEqual(["objet-A.ttf", "objet-Parti.ttf"]);
    expect(faceObjetManque("objet-A.ttf")).toBe(false);
    expect(faceObjetManque("objet-Parti.ttf")).toBe(true);
    expect(tourDeFace()).toBeGreaterThan(avant);
    const familles = [...posees].map((f) => (f as { family: string }).family);
    expect(familles).toContain("colophon-objet-A");
    const options = [...posees].map((f) => (f as { options: Record<string, string> }).options);
    expect(options[0].featureSettings).toContain('"kern" 0');

    // Déjà connues : rien n'est relu.
    await chargerFacesObjet(["objet-A.ttf"], lire);
    expect(lus).toHaveLength(2);

    oublierFacesObjet();
    expect(posees.size).toBe(0);
    expect(faceObjetManque("objet-Parti.ttf")).toBe(false);
    await chargerFacesObjet(["objet-A.ttf"], lire);
    expect(lus).toHaveLength(3);
    oublierFacesObjet();
  });
});
