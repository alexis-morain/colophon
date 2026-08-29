// Ce que le sélecteur de gabarits montre, et ce qu'il replie.
//
// Le test qui compte est le premier : **aucun nom du catalogue n'échappe à
// la grammaire**. C'est lui qui rend acceptable de relire ici un nom que le
// moteur sait écrire, parce que le jour où la grammaire bouge, il rougit —
// au lieu qu'un `g_1x2f_1x2f` s'affiche à quelqu'un.
//
// Les deux suivants tiennent la table des familles historiques, qui est le
// seul endroit de l'application qui redit une forme du moteur : la
// disposition déclarée doit compter autant de cases que le dump, et les
// répartir des deux côtés du pli comme le dump les place.

import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import { Dump, setGeometrie } from "./geometrie";

setGeometrie(fixture as unknown as Dump);

import {
  cases,
  choixOfferts,
  cleDeForme,
  formeDe,
  libelleForme,
  libelleGabarit,
} from "./gabarit";
import { setLangue } from "./i18n";

const dump = fixture as unknown as Dump;
const avecPhotos = dump.ordre.filter(([, cap]) => cap > 0);

/** Combien de cases ce gabarit pose de chaque côté du pli, d'après les
 *  rectangles du moteur. Un rectangle appartient à la page dont il occupe
 *  le centre ; le pli est au milieu du média, fond perdu compris. */
function parPage(nom: string): [number, number] {
  const pli = dump.media.w / 2;
  let gauche = 0;
  let droite = 0;
  for (const [x, , w] of dump.templates[nom].slots) {
    if (x + w / 2 < pli) gauche += 1;
    else droite += 1;
  }
  return [gauche, droite];
}

describe("la grammaire des noms de gabarits", () => {
  it("lit tous les noms du catalogue offert", () => {
    for (const [nom] of avecPhotos) {
      expect(formeDe(nom), nom).not.toBeNull();
    }
  });

  it("déclare exactement les cases que le dump pose", () => {
    for (const [nom, cap] of avecPhotos) {
      const f = formeDe(nom)!;
      expect(cases(f.a) + cases(f.b), nom).toBe(cap);
    }
  });

  it("répartit ces cases des deux côtés du pli comme le dump", () => {
    for (const [nom] of avecPhotos) {
      const f = formeDe(nom)!;
      // La paire est normalisée, donc sans côté : ce qui se compare est
      // l'ensemble des deux nombres, pas leur ordre.
      expect([cases(f.a), cases(f.b)].sort(), nom).toEqual(
        parPage(nom).sort(),
      );
    }
  });

  it("replie un verso sur son recto", () => {
    for (const [nom] of avecPhotos) {
      if (!nom.endsWith("_verso")) continue;
      const recto = nom.slice(0, -"_verso".length);
      expect(cleDeForme(formeDe(nom)!), nom).toBe(cleDeForme(formeDe(recto)!));
    }
  });

  it("replie une bande de légende, qui n'est pas une disposition", () => {
    expect(cleDeForme(formeDe("g_1x2f_1x2f")!)).toBe(
      cleDeForme(formeDe("g_1x2f_1x2f_b8")!),
    );
  });

  it("replie une forme de cellule, que la vignette montre déjà", () => {
    // Une lettre de ratio ne change pas la disposition : quatre photos,
    // deux par page, que les cases soient carrées ou panoramiques.
    expect(cleDeForme(formeDe("g_1x2c_1x2c")!)).toBe(
      cleDeForme(formeDe("g_1x2f_1x2f")!),
    );
    // Et une famille historique tombe dans la disposition qu'elle dessine.
    expect(cleDeForme(formeDe("quad_pano")!)).toBe(
      cleDeForme(formeDe("g_1x2f_1x2f")!),
    );
    expect(cleDeForme(formeDe("duo_portrait")!)).toBe(
      cleDeForme(formeDe("duo")!),
    );
  });

  it("ne lit pas les planches sans photo, qui n'ont pas de cases", () => {
    for (const nom of ["texte", "garde", "colophon", "vide"]) {
      expect(formeDe(nom), nom).toBeNull();
    }
  });
});

describe("le libellé d'une disposition", () => {
  it("dit ce que la planche porte, sans en nommer le côté", () => {
    setLangue("fr");
    const l = (nom: string) => libelleForme(formeDe(nom)!);
    expect(l("full1")).toBe("Pleine page");
    expect(l("solo_pano")).toBe("Photo cadrée");
    expect(l("duo")).toBe("Une par page");
    expect(l("g_v_1x2f")).toBe("Deux en colonne, sur une page");
    expect(l("g_v_2x1c")).toBe("Deux côte à côte, sur une page");
    expect(l("g_v_2x2f")).toBe("Quatre en grille, sur une page");
    expect(l("g_1x2f_1x2f")).toBe("Deux en colonne par page");
    expect(l("g_p_p_b8")).toBe("Deux pleines pages");
    expect(l("g_p_1x2c")).toBe("Une pleine page, deux en colonne");
    expect(l("g_1x1f_1x3c")).toBe("Une, et trois en colonne");
  });

  it("garde les planches sans photo au dictionnaire", () => {
    setLangue("fr");
    expect(libelleGabarit("texte")).toBe("Planche de texte");
    expect(libelleGabarit("garde")).toBe("Page de garde");
    // Un nom qu'aucune des deux routes ne reconnaît s'affiche tel quel :
    // le défaut honnête, celui qui se voit.
    expect(libelleGabarit("mystere")).toBe("mystere");
  });

  it("existe dans les deux langues", () => {
    setLangue("en");
    expect(libelleForme(formeDe("g_1x2f_1x2f")!)).toBe("Two stacked per page");
    expect(libelleForme(formeDe("full1")!)).toBe("Full page");
    setLangue("fr");
  });
});

describe("ce que le sélecteur offre", () => {
  // Le pire cas réel mesuré le 29/08 : une planche de quatre photos, dont
  // le moteur jugeait 171 gabarits compatibles.
  const notes = avecPhotos
    .filter(([, cap]) => cap <= 4)
    .map(([nom]) => [nom, 1] as [string, number]);

  it("montre une disposition et pas un gabarit", () => {
    const choix = choixOfferts(notes, "quad");
    expect(notes.length).toBeGreaterThan(150);
    expect(choix.length).toBeLessThan(40);
    const cles = choix.map((c) => c.cle);
    expect(new Set(cles).size).toBe(cles.length);
  });

  it("nomme chaque entrée, jamais un nom de fichier", () => {
    for (const c of choixOfferts(notes, "quad")) {
      expect(c.libelle, c.cle).not.toMatch(/^g_/);
      expect(c.libelle.length, c.cle).toBeGreaterThan(0);
    }
  });

  it("préfère la variante sans bande de légende", () => {
    // Le Composer n'en pose aucune : garder la bande par défaut changerait
    // la mise en page de toutes les légendes pour un choix qui ne parle que
    // de cadres.
    const choix = choixOfferts(
      [
        ["g_1x2f_1x2f_b8", 1.0],
        ["g_1x2f_1x2f", 1.4],
      ],
      "quad",
    );
    expect(choix.find((c) => c.capacite === 4)?.template).toBe("g_1x2f_1x2f");
  });

  it("pose, à bande égale, celle que ces photos cadrent le mieux", () => {
    const choix = choixOfferts(
      [
        ["g_1x2c_1x2c", 1.3],
        ["g_1x2f_1x2f", 1.05],
        ["g_1x2l_1x2l", 1.2],
      ],
      "quad",
    );
    expect(choix[0].template).toBe("g_1x2f_1x2f");
  });

  it("garde la disposition de la planche, même si le moteur la refuse", () => {
    // L'état d'une planche n'est jamais un cul-de-sac : un recadrage a pu
    // la rendre inapte à son propre gabarit, il doit rester visible.
    const choix = choixOfferts([["duo", 1]], "quad_pano");
    expect(choix.map((c) => c.cle)).toContain(
      cleDeForme(formeDe("quad_pano")!),
    );
  });

  it("trie par nombre de photos décroissant", () => {
    const caps = choixOfferts(notes, "quad").map((c) => c.capacite);
    expect(caps).toEqual([...caps].sort((a, b) => b - a));
  });
});
