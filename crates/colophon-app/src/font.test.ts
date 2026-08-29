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

beforeAll(() => {
  (globalThis as unknown as { document: unknown }).document = {
    createElement: () => ({ getContext: () => ctx }),
    fonts: { load: () => Promise.resolve([]) },
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
