// L'écran Envoi, sans DOM : quel imprimeur il nomme, ce que fait son bouton,
// et la phrase qu'il dit après avoir préparé le dossier. Vitest tourne sans
// `navigator`, langue anglaise par défaut : chaque cas pose la sienne.

import { afterEach, describe, expect, it } from "vitest";
import type { Album } from "./album";
import type { Defaut, Preparation, Printer, PrevolReport } from "./bridge";
import { setLangue } from "./i18n";
import {
  PROFIL_ENVOI,
  PROFIL_LIBRE,
  actionEnvoi,
  actionsDefaut,
  imprimeurAffiche,
  phrasePreparation,
  ppiDe,
} from "./envoi";

afterEach(() => setLangue("en"));

const profil = (id: string, nom: string, fichiers: "un" | "deux"): Printer => ({
  id,
  nom,
  pdf_x: "x4",
  espace: "rgb",
  bleed_mm: { haut: 3, bas: 3, exterieur: 3, dos: 3 },
  rempli_mm: 0,
  debord_mm: 0,
  mors_mm: 0,
  safe_mm: 7,
  fichiers,
  pages_simples: true,
  dos: { mode: "fourni" },
  pages_min: 24,
  pages_max: 800,
  pas_pagination: 2,
  min_ppi: 250,
  certitude: "confirme",
  reserves: [],
});

/** Les quatre profils du moteur, dans son ordre. */
const PROFILS: Printer[] = [
  profil("cloudprinter", "Cloudprinter", "deux"),
  profil("prodigi", "Prodigi", "un"),
  profil("lulu", "Lulu", "deux"),
  profil("generique", "Imprimeur local (PDF sans contrainte)", "un"),
];

describe("imprimeurAffiche", () => {
  it("ne nomme que Cloudprinter, ou le PDF sans contrainte quand on l'a choisi", () => {
    expect(PROFIL_ENVOI).toBe("cloudprinter");
    expect(PROFIL_LIBRE).toBe("generique");
    expect(imprimeurAffiche(PROFILS, "cloudprinter")?.id).toBe("cloudprinter");
    expect(imprimeurAffiche(PROFILS, "generique")?.id).toBe("generique");
  });

  it("ne montre jamais Prodigi ni Lulu, qui restent au moteur et à la CLI", () => {
    // Un profil hérité d'ailleurs retombe sur l'imprimeur de l'écran.
    expect(imprimeurAffiche(PROFILS, "prodigi")?.id).toBe("cloudprinter");
    expect(imprimeurAffiche(PROFILS, "lulu")?.id).toBe("cloudprinter");
  });

  it("attend la liste plutôt que d'inventer un imprimeur", () => {
    expect(imprimeurAffiche(null, "cloudprinter")).toBeNull();
  });
});

describe("actionEnvoi", () => {
  it("prépare un dossier chez qui relie deux fichiers", () => {
    setLangue("fr");
    const a = actionEnvoi(PROFILS[0]);
    expect(a.prepare).toBe(true);
    expect(a.texte).toBe("Préparer pour Cloudprinter");
  });

  it("enregistre un PDF, comme avant, sous le profil sans contrainte", () => {
    setLangue("fr");
    const a = actionEnvoi(PROFILS[3]);
    expect(a.prepare).toBe(false);
    expect(a.texte).toBe("Enregistrer le PDF d’impression");
  });
});

const rapport = (bloquants: number): PrevolReport =>
  ({
    album: "Corse",
    profil: "cloudprinter",
    ok: bloquants === 0,
    bloquants,
    avertissements: 0,
    defauts: [],
  }) as unknown as PrevolReport;

const preparation = (bloquants: number): Preparation => ({
  dossier: "Corse – Cloudprinter",
  fichiers: ["album-print.pdf", "album-cover.pdf", "export.json", "fiche.txt"],
  rapport: rapport(bloquants),
});

describe("phrasePreparation", () => {
  it("dit en une phrase que le dossier est prêt, et lequel", () => {
    setLangue("fr");
    expect(phrasePreparation(preparation(0))).toBe(
      "Dossier « Corse – Cloudprinter » prêt : les deux PDF relus, rien ne s’oppose à la commande.",
    );
  });

  it("dit ce que le contrôle relu a trouvé, au singulier et au pluriel", () => {
    setLangue("fr");
    expect(phrasePreparation(preparation(1))).toContain("un défaut");
    expect(phrasePreparation(preparation(3))).toContain("3 défauts");
    setLangue("en");
    expect(phrasePreparation(preparation(3))).toContain("3 defects");
  });
});

describe("actionsDefaut", () => {
  // Planche 10, case 1 : IMG_2217, la photo que le prévol nomme.
  const album = {
    spreads: Array.from({ length: 12 }, (_, i) => ({
      template: "g",
      slots:
        i === 9
          ? [
              { src: "IMG_2216.jpg", focal: [0.5, 0.5] },
              { src: "IMG_2217.jpg", focal: [0.5, 0.5] },
            ]
          : [],
    })),
  } as unknown as Album;
  const resolution: Defaut = {
    regle: "resolution",
    bloquant: true,
    planche: 10,
    case: 1,
    src: "IMG_2217.jpg",
    cause: "IMG_2217.jpg imprimerait à 130 ppi dans cette case, Cloudprinter exige 250",
    remede: "mettez-la dans une case plus petite, remplacez-la, ou retirez-la",
  };

  it("voir, remplacer et retirer quand la réserve a des photos", () => {
    expect(actionsDefaut(resolution, album, false)).toEqual(["voir", "remplacer", "retirer"]);
  });

  it("retirer reste quand la réserve est vide", () => {
    expect(actionsDefaut(resolution, album, true)).toEqual(["voir", "retirer"]);
  });

  it("n'invente rien pour un défaut sans case", () => {
    const { case: _, ...sansCase } = resolution;
    expect(actionsDefaut(sansCase, album, false)).toEqual([]);
  });

  it("ne donne aucune action neuve à une autre règle", () => {
    const pli: Defaut = { ...resolution, regle: "objet_pli" };
    expect(actionsDefaut(pli, album, false)).toEqual([]);
  });

  it("se tait quand la case ne porte plus la photo du rapport", () => {
    // Le rapport lit le disque : après une édition non enregistrée, la case
    // 1 peut porter une autre photo, et retirer viserait la mauvaise.
    expect(actionsDefaut({ ...resolution, case: 0 }, album, false)).toEqual([]);
  });
});

describe("ppiDe", () => {
  it("lit la résolution dans la cause du moteur", () => {
    const d = {
      regle: "resolution",
      bloquant: true,
      cause: "IMG_2217.jpg imprimerait à 130 ppi dans cette case, Cloudprinter exige 250",
      remede: "",
    };
    expect(ppiDe(d)).toBe(130);
    expect(ppiDe({ ...d, cause: "autre chose" })).toBeNull();
  });
});
