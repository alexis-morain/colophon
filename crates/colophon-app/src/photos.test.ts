// L'alerte de qualité (chantier G) : trois alertes typées, lues d'une seule
// règle, `badgesDe`, et une seule source pour ce que l'original mesure — le
// relevé de l'album. Sans relevé, aucune alerte : l'écran ne devine pas la
// taille d'une photo d'après sa vignette, ni son flou d'après rien.
//
// Le « sombre » lit encore la vignette au travers du réglage
// (`thumbs.test.ts` en tient l'arithmétique) : ce fichier lui prête le même
// canevas d'une seule teinte.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { MIN_EFFECTIVE_PPI, Rect } from "./album";
import type { MesurePhoto, ReleveAlbum } from "./bridge";
import { setLangue } from "./i18n";
import {
  alertesDeReserve,
  badgesDe,
  chargerReleve,
  nomDAlerte,
  releveCourant,
} from "./photos";
import { resetThumbs } from "./thumbs";

const TAILLE = 64;

function poserCanvas(luma: number): void {
  const data = new Uint8ClampedArray(TAILLE * TAILLE * 4);
  for (let i = 0; i < data.length; i += 4) {
    data[i] = luma;
    data[i + 1] = luma;
    data[i + 2] = luma;
    data[i + 3] = 255;
  }
  (globalThis as any).document = {
    createElement: () => ({
      width: 0,
      height: 0,
      getContext: () => ({
        drawImage: () => {},
        getImageData: () => ({ data }),
      }),
    }),
  };
}

const image = () =>
  ({ complete: true, naturalWidth: 800, naturalHeight: 600 }) as HTMLImageElement;

/** 40 × 30 mm : à 4000 px de large, 2540 ppi ; à 300 px, 190. */
const CASE: Rect = { x: 0, y: 0, w: 40, h: 30 };

const mesure = (m: Partial<MesurePhoto> = {}): MesurePhoto => ({
  largeur: 4000,
  hauteur: 3000,
  nettete: 500,
  exposition: 0.8,
  ...m,
});

const releve = (
  photos: Record<string, MesurePhoto>,
  seuil_flou: number | null = 100,
): ReleveAlbum => ({ photos, seuil_flou });

const codes = (src: string, r: ReleveAlbum | null) =>
  badgesDe(src, image(), CASE, 1, 1, undefined, r).alertes.map((a) => a.code);

describe("badgesDe, les trois alertes", () => {
  beforeEach(() => {
    resetThumbs();
    poserCanvas(180); // une photo claire : seul ce qu'on règle déclenche
  });
  afterEach(() => {
    delete (globalThis as any).document;
  });

  it("sous_resolution : le ppi de la case, sur la taille de l'original", () => {
    const r = releve({ "petite.jpg": mesure({ largeur: 300, hauteur: 225 }) });
    const b = badgesDe("petite.jpg", image(), CASE, 1, 1, undefined, r);
    expect(b.alertes).toEqual([{ code: "sous_resolution", ppi: 191 }]);
    // La vignette fait 800 px et n'en dit rien : la même photo, grande dans
    // le relevé, ne porte rien, et un zoom ×1,5 la fait passer dessous.
    const grande = releve({ "grande.jpg": mesure({ largeur: 400, hauteur: 300 }) });
    expect(codes("grande.jpg", grande)).toEqual([]);
    const zoomee = badgesDe("grande.jpg", image(), CASE, 1, 1.5, undefined, grande);
    expect(zoomee.alertes).toEqual([{ code: "sous_resolution", ppi: 169 }]);
    expect(169).toBeLessThan(MIN_EFFECTIVE_PPI);
  });

  it("sombre : la vignette, au travers du réglage", () => {
    poserCanvas(30);
    const r = releve({ "nuit.jpg": mesure() });
    expect(codes("nuit.jpg", r)).toEqual(["sombre"]);
    const rattrapee = badgesDe("nuit.jpg", image(), CASE, 1, 1, { expo: 1.5, contraste: 0, nb: false }, r);
    expect(rattrapee.alertes).toEqual([]);
  });

  it("floue : au plus le seuil du dossier, jamais sans seuil", () => {
    expect(codes("bougee.jpg", releve({ "bougee.jpg": mesure({ nettete: 100 }) }))).toEqual([
      "floue",
    ]);
    expect(codes("nette.jpg", releve({ "nette.jpg": mesure({ nettete: 100.5 }) }))).toEqual([]);
    // Un petit dossier n'a pas de décile : rien n'y est flou.
    expect(codes("bougee.jpg", releve({ "bougee.jpg": mesure({ nettete: 1 }) }, null))).toEqual(
      [],
    );
  });

  it("les trois ensemble, dans un ordre fixe", () => {
    poserCanvas(30);
    const r = releve({ "tout.jpg": mesure({ largeur: 300, hauteur: 225, nettete: 3 }) });
    expect(codes("tout.jpg", r)).toEqual(["sous_resolution", "sombre", "floue"]);
  });

  it("aucune alerte sans relevé", () => {
    poserCanvas(30); // sombre, et une vignette de 800 px dans une case de 40 mm
    expect(codes("nuit.jpg", null)).toEqual([]);
    // Un relevé qui ne connaît pas la photo ne dit rien d'elle non plus.
    expect(codes("nuit.jpg", releve({ "autre.jpg": mesure({ nettete: 1 }) }))).toEqual([]);
  });

  it("sans pixels, ce que le relevé sait suffit", () => {
    const r = releve({ "p.jpg": mesure({ largeur: 300, hauteur: 225, nettete: 3 }) });
    const b = badgesDe("p.jpg", null, CASE, 1, 1, undefined, r);
    expect(b.alertes.map((a) => a.code)).toEqual(["sous_resolution", "floue"]);
    expect(b.sansMarge).toBe(false);
  });
});

describe("la réserve, sur une demi-page", () => {
  const CARRE = { w: 210, h: 210 };

  it("une photo dont le grand côté ne tient pas une demi-page à 250 ppi", () => {
    // Demi-page du carré 21 : 210 × 105 mm. 2000 px sur 210 mm : 242 ppi.
    const r = releve({
      "juste.jpg": mesure({ largeur: 2000, hauteur: 1500 }),
      "assez.jpg": mesure({ largeur: 2400, hauteur: 1600 }),
      "debout.jpg": mesure({ largeur: 1500, hauteur: 2000 }),
    });
    expect(alertesDeReserve("juste.jpg", CARRE, r)).toEqual([
      { code: "sous_resolution", ppi: 242 },
    ]);
    expect(alertesDeReserve("assez.jpg", CARRE, r)).toEqual([]);
    expect(alertesDeReserve("debout.jpg", CARRE, r)).toEqual([
      { code: "sous_resolution", ppi: 242 },
    ]);
  });

  it("floue, et rien sans relevé", () => {
    const r = releve({ "f.jpg": mesure({ nettete: 2 }) });
    expect(alertesDeReserve("f.jpg", CARRE, r)).toEqual([{ code: "floue" }]);
    expect(alertesDeReserve("f.jpg", CARRE, null)).toEqual([]);
  });
});

describe("le nom d'une alerte, pour la voix", () => {
  it("dit le ppi et le plancher", () => {
    setLangue("fr");
    expect(nomDAlerte({ code: "sous_resolution", ppi: 220 })).toBe("220 ppi, sous 250");
    expect(nomDAlerte({ code: "floue" })).toBe("floue");
    setLangue("en");
    expect(nomDAlerte({ code: "sous_resolution", ppi: 220 })).toBe("220 ppi, under 250");
    expect(nomDAlerte({ code: "sombre" })).toBe("dark");
  });
});

describe("le relevé, une fois par album", () => {
  it("le dernier album ouvert gagne, même si l'ancien répond après", async () => {
    let lent!: (r: ReleveAlbum) => void;
    const ancien = chargerReleve(() => new Promise((ok) => (lent = ok)));
    expect(releveCourant()).toBeNull();
    await chargerReleve(async () => releve({ "b.jpg": mesure() }));
    lent(releve({ "a.jpg": mesure() }));
    await ancien;
    expect(Object.keys(releveCourant()!.photos)).toEqual(["b.jpg"]);
  });

  it("un relevé qui échoue laisse l'album sans alerte", async () => {
    await chargerReleve(async () => {
      throw new Error("pas de moteur");
    });
    expect(releveCourant()).toBeNull();
  });
});
