// L'écran Envoi, sans DOM : quel imprimeur il nomme, ce que fait son bouton,
// et la phrase qu'il dit après avoir préparé le dossier. Vitest tourne sans
// `navigator`, langue anglaise par défaut : chaque cas pose la sienne.

import { afterEach, describe, expect, it } from "vitest";
import type { Preparation, Printer, PrevolReport } from "./bridge";
import { setLangue } from "./i18n";
import {
  PROFIL_ENVOI,
  PROFIL_LIBRE,
  actionEnvoi,
  imprimeurAffiche,
  phrasePreparation,
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
