// La commande, sans DOM : quand « Commander » s'affiche, le prix, les états
// du relais, la confirmation et la machine du parcours. Vitest tourne en
// anglais par défaut : chaque cas pose sa langue.

import { afterEach, describe, expect, it } from "vitest";
import type { Preparation, PrevolReport } from "./bridge";
import {
  IntentionVue,
  Offre,
  Parcours,
  Pays,
  aRelire,
  annulable,
  annulableIntention,
  avancer,
  commandePossible,
  formatPrix,
  niveauChoisi,
  nomNiveau,
  part,
  phraseConfirmation,
  phraseErreur,
  phraseEtat,
  phraseIntention,
  prixDeCentimes,
  visibles,
} from "./commande";
import { setLangue } from "./i18n";

afterEach(() => setLangue("en"));

const preparation = (ok: boolean): Preparation => ({
  dossier: "Album – Cloudprinter",
  fichiers: ["album-print.pdf", "album-cover.pdf"],
  rapport: { ok, bloquants: ok ? 0 : 1, defauts: [] } as unknown as PrevolReport,
});

const intention = (etat: string, dernier_code: number | null = null): IntentionVue => ({
  id: "zNRsXsjGDfnOhog10i579Q",
  mode: "sandbox",
  etat,
  dernier_code,
});

// La grille de la mesure S-s1 du 09/10.
const OFFRE: Offre = {
  id: "zNRsXsjGDfnOhog10i579Q",
  mode: "sandbox",
  grille: [
    { niveau: "cp_ground", transporteur: "Fedex - Regional Economy", prix_ttc_centimes: 4600, delai_jours: 4 },
    { niveau: "cp_fast", transporteur: "FedEx - International Priority", prix_ttc_centimes: 5250, delai_jours: 4 },
  ],
  expire: "2026-10-11T16:20:51Z",
};

const PAYS: Pays[] = [
  { code: "DE", nom: "Germany", require_state: false },
  { code: "FR", nom: "France", require_state: false },
  { code: "ES", nom: "Spain", require_state: true, etats: [{ code: "M", nom: "Madrid" }] },
];

describe("commandePossible", () => {
  it("demande un relais joignable, un dossier de cette ouverture et un contrôle vert", () => {
    expect(commandePossible(true, preparation(true))).toBe(true);
  });

  it("reste faux sans relais, quoi qu'il arrive", () => {
    expect(commandePossible(false, preparation(true))).toBe(false);
    expect(commandePossible(false, preparation(false))).toBe(false);
    expect(commandePossible(false, null)).toBe(false);
  });

  it("reste faux avec un relais, sans dossier neuf ou sur un contrôle rouge", () => {
    expect(commandePossible(true, null)).toBe(false);
    expect(commandePossible(true, preparation(false))).toBe(false);
  });
});

describe("le prix", () => {
  it("s'écrit au centime, dans la langue de l'écran", () => {
    expect(formatPrix("20.9584", "EUR", "fr")).toBe("20,96 €");
    expect(formatPrix("20.9584", "EUR", "en")).toBe("€20.96");
    expect(formatPrix("11.8333", "EUR", "fr")).toBe("11,83 €");
    expect(formatPrix("1234.5", "EUR", "fr")).toBe("1 234,50 €");
  });

  it("de la grille vient en centimes entiers, sans flottant", () => {
    expect(prixDeCentimes(4600, "fr")).toBe("46,00 €");
    expect(prixDeCentimes(5250, "en")).toBe("€52.50");
    expect(prixDeCentimes(1005, "fr")).toBe("10,05 €");
    expect(prixDeCentimes(29, "fr")).toBe("0,29 €");
  });

  it("porte le nom du niveau, et le transporteur quand il est dit", () => {
    setLangue("fr");
    expect(nomNiveau(OFFRE.grille[0])).toBe("Standard · Fedex - Regional Economy");
    expect(nomNiveau({ ...OFFRE.grille[1], transporteur: null })).toBe("Express");
    expect(nomNiveau({ niveau: "cp_jamais_vu", transporteur: null, prix_ttc_centimes: 1, delai_jours: null })).toBe(
      "cp_jamais_vu",
    );
  });
});

describe("les états de Cloudprinter", () => {
  it("ont chacun leur phrase, l'inconnu compris", () => {
    setLangue("fr");
    const codes = [null, 1, 5, 7, 10, 15, 30, 45, 100, 500, 501];
    const phrases = codes.map((c) => phraseEtat(c));
    // Une phrase par famille : 5 et 7, 10 et 15, 30 et 45 se rejoignent.
    expect(new Set(phrases).size).toBe(8);
    expect(phraseEtat(501)).toContain("sandbox");
    expect(phraseEtat(8)).toContain("8");
    setLangue("en");
    expect(phraseEtat(100)).not.toBe("commande.etat.expediee");
  });

  it("s'annulent sous 30 seulement", () => {
    for (const c of [null, 1, 5, 10, 15, 29]) expect(annulable(c), String(c)).toBe(true);
    for (const c of [30, 45, 100, 500, 501]) expect(annulable(c), String(c)).toBe(false);
  });
});

describe("les états du relais", () => {
  const ETATS = [
    "attente_fichiers",
    "attente_paiement",
    "payee",
    "commandee",
    "remboursement_en_cours",
    "echec_remboursee",
    "annulation_en_cours",
    "annulee",
    "purgee",
  ];

  it("ont chacun leur phrase, en français et en anglais", () => {
    for (const langue of ["fr", "en"] as const) {
      setLangue(langue);
      const phrases = ETATS.map((e) => phraseIntention(intention(e)));
      expect(new Set(phrases).size, langue).toBe(9);
      for (const p of phrases) expect(p, langue).not.toMatch(/^commande\./);
    }
  });

  it("disent « remboursé » quand la commande n'a pas pu partir", () => {
    setLangue("fr");
    expect(phraseIntention(intention("echec_remboursee"))).toContain("remboursé");
    setLangue("en");
    expect(phraseIntention(intention("echec_remboursee"))).toContain("refunded");
  });

  it("donnent la référence et l'état chez Cloudprinter une fois commandée", () => {
    setLangue("fr");
    const p = phraseIntention(intention("commandee", 501));
    expect(p).toContain("zNRsXsjGDfnOhog10i579Q");
    expect(p).toContain(phraseEtat(501));
  });

  it("disent un état inconnu d'une phrase neutre, sans lever", () => {
    setLangue("fr");
    expect(() => phraseIntention(intention("expiree"))).not.toThrow();
    expect(phraseIntention(intention("expiree"))).toContain("expiree");
    expect(phraseIntention(intention("__proto__"))).toContain("__proto__");
  });

  it("offrent Annuler sur une commande passée sous 30, et nulle part ailleurs", () => {
    expect(annulableIntention(intention("commandee", null))).toBe(true);
    expect(annulableIntention(intention("commandee", 15))).toBe(true);
    expect(annulableIntention(intention("commandee", 30))).toBe(false);
    for (const e of ETATS.filter((x) => x !== "commandee")) {
      expect(annulableIntention(intention(e, 1)), e).toBe(false);
    }
  });

  it("se relisent tant qu'elles bougent seules, et la liste tait celles restées en route", () => {
    const toutes = ETATS.map((e, i) => ({ ...intention(e), id: String(i) }));
    expect(aRelire(toutes)).toEqual(["1", "2", "4", "6"]);
    expect(visibles(toutes).map((i) => i.etat)).not.toContain("attente_fichiers");
    expect(visibles(toutes)).toHaveLength(8);
  });
});

describe("la confirmation", () => {
  it("dit le prix et le mode en toutes lettres", () => {
    setLangue("fr");
    expect(phraseConfirmation("46,00 €", "sandbox")).toBe(
      "Commander pour 46,00 € TTC, livraison comprise, en sandbox : la page de paiement est un test, rien ne sera débité ni imprimé.",
    );
    expect(phraseConfirmation("46,00 €", "reel")).toBe(
      "Commander pour 46,00 € TTC, livraison comprise : vous paierez par carte sur la page de Stripe, qui s’ouvre dans votre navigateur.",
    );
  });

  it("dit le 409 et les refus connus du relais avec leur phrase", () => {
    setLangue("fr");
    expect(phraseErreur(new Error("relais : trop_tard (409)"))).toBe(
      "La production a commencé, l’annulation n’est plus possible.",
    );
    expect(phraseErreur("relais : devis_expire (409)")).toContain("expiré");
    expect(phraseErreur("relais : commander_indisponible (503)")).toContain("indisponible");
    expect(phraseErreur(new Error("relais injoignable : dns"))).toContain("injoignable");
  });
});

describe("le parcours", () => {
  const suite = (p: Parcours, ...es: Parameters<typeof avancer>[1][]) => es.reduce(avancer, p);

  it("va du bouton au paiement, dans l'ordre du contrat", () => {
    let p: Parcours = avancer({ etape: "repos" }, { type: "commander", pays: PAYS });
    expect(p).toMatchObject({ etape: "pays", choisi: "FR", region: null });
    p = suite(p, { type: "offre", offre: OFFRE });
    expect(p).toMatchObject({ etape: "grille", niveau: "cp_ground" });
    p = suite(p, { type: "niveau", niveau: "cp_fast" }, { type: "envoyer" });
    expect(p.etape).toBe("envoi");
    p = suite(p, { type: "progres", progres: { fichier: "interieur", envoyes: 47_490_052, total: 94_980_105 } });
    if (p.etape !== "envoi") throw new Error(p.etape);
    expect(part(p.progres.interieur)).toBeCloseTo(0.5, 3);
    expect(part(p.progres.couverture)).toBe(0);
    p = suite(p, { type: "envoye" });
    expect(p.etape).toBe("confirmation");
    if (p.etape !== "confirmation") throw new Error(p.etape);
    expect(niveauChoisi(p).prix_ttc_centimes).toBe(5250);
    p = suite(p, { type: "payer" }, { type: "ouvert" });
    expect(p.etape).toBe("attente");
    // Encore en attente : rien ne bouge ; payée : la liste prend le relais.
    expect(avancer(p, { type: "etat", intention: intention("attente_paiement") })).toBe(p);
    expect(avancer(p, { type: "etat", intention: { ...intention("payee"), id: "autre" } })).toBe(p);
    expect(suite(p, { type: "etat", intention: intention("payee") })).toEqual({ etape: "repos" });
  });

  it("choisit le premier pays sans la France, et l'État quand il en faut un", () => {
    const p = avancer({ etape: "repos" }, { type: "commander", pays: [PAYS[0], PAYS[2]] });
    expect(p).toMatchObject({ choisi: "DE", region: null });
    expect(avancer(p, { type: "pays", code: "ES" })).toMatchObject({ choisi: "ES", region: "M" });
    expect(avancer(p, { type: "pays", code: "XX" })).toBe(p);
    expect(avancer({ etape: "repos" }, { type: "commander", pays: [] })).toEqual({ etape: "repos" });
  });

  it("rouvre ce qu'un échec ou un retour a laissé, sans renvoyer les fichiers", () => {
    const grille = suite({ etape: "repos" }, { type: "commander", pays: PAYS }, { type: "offre", offre: OFFRE });
    const envoi = avancer(grille, { type: "envoyer" });
    expect(avancer(envoi, { type: "echec" }).etape).toBe("grille");
    expect(avancer(envoi, { type: "retour" })).toBe(envoi);
    const confirmation = avancer(envoi, { type: "envoye" });
    const pret = avancer(confirmation, { type: "retour" });
    expect(pret.etape).toBe("pret");
    expect(avancer(pret, { type: "confirmer" }).etape).toBe("confirmation");
    const paiement = avancer(confirmation, { type: "payer" });
    expect(avancer(paiement, { type: "echec" }).etape).toBe("pret");
    // En attente, on peut rouvrir la page de paiement.
    const attente = avancer(paiement, { type: "ouvert" });
    expect(avancer(attente, { type: "payer" }).etape).toBe("paiement");
    expect(avancer(grille, { type: "retour" })).toEqual({ etape: "repos" });
  });

  it("ignore un événement qui ne concerne pas l'étape", () => {
    const repos: Parcours = { etape: "repos" };
    for (const e of [
      { type: "offre", offre: OFFRE },
      { type: "envoye" },
      { type: "payer" },
      { type: "ouvert" },
      { type: "progres", progres: { fichier: "interieur", envoyes: 1, total: 2 } },
    ] as const) {
      expect(avancer(repos, e), e.type).toBe(repos);
    }
    const p = avancer(repos, { type: "commander", pays: PAYS });
    expect(avancer(p, { type: "offre", offre: { ...OFFRE, grille: [] } })).toBe(p);
  });
});
