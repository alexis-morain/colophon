// La commande, sans DOM : quand « Commander » s'affiche, le prix, les états,
// l'expiration du devis et la confirmation. Vitest tourne en anglais par
// défaut : chaque cas pose sa langue.

import { afterEach, describe, expect, it } from "vitest";
import type { Preparation, PrevolReport } from "./bridge";
import {
  Devis,
  Vue,
  annulable,
  commandePossible,
  devisExpire,
  formatPrix,
  phraseConfirmation,
  phraseErreur,
  phraseEtat,
  totalTTC,
} from "./commande";
import { setLangue } from "./i18n";

afterEach(() => setLangue("en"));

const vue = (cle: boolean, depot: boolean): Vue => ({
  cle: cle ? { fin: "ab12", mode: "Sandbox" } : null,
  depot: depot ? { endpoint: "https://x", region: "auto", bucket: "b" } : null,
  commandes: [],
});

const preparation = (ok: boolean): Preparation => ({
  dossier: "Album – Cloudprinter",
  fichiers: ["album-print.pdf", "album-cover.pdf"],
  rapport: { ok, bloquants: ok ? 0 : 1, defauts: [] } as unknown as PrevolReport,
});

/** Le devis du sandbox, du 03/10. */
const DEVIS: Devis = {
  prix: "11.8333",
  tva: "2.3667",
  devise: "EUR",
  expire_date: "2026-10-06T01:37:58.083165Z",
  production_jours: 4,
  expeditions: [
    {
      poids_g: "627",
      offres: [
        {
          quote: "a1b2",
          niveau: "cp_limited",
          service: "Limited",
          transporteur: "Chronopost - France",
          prix: "5.6320",
          tva: "1.1264",
          devise: "EUR",
        },
      ],
    },
  ],
};

describe("commandePossible", () => {
  it("demande la clé, le dépôt, un dossier de cette ouverture et un contrôle vert", () => {
    expect(commandePossible(vue(true, true), preparation(true))).toBe(true);
    expect(commandePossible(vue(false, true), preparation(true))).toBe(false);
    expect(commandePossible(vue(true, false), preparation(true))).toBe(false);
    expect(commandePossible(vue(true, true), null)).toBe(false);
    expect(commandePossible(vue(true, true), preparation(false))).toBe(false);
    expect(commandePossible(null, preparation(true))).toBe(false);
  });
});

describe("le prix", () => {
  it("additionne articles, TVA et expédition sans flottant qui dérive", () => {
    expect(totalTTC(DEVIS, DEVIS.expeditions[0].offres[0])).toBe("20.9584");
    const o = { ...DEVIS.expeditions[0].offres[0], prix: "0.1", tva: "0.2" };
    expect(totalTTC({ ...DEVIS, prix: "0", tva: "0" }, o)).toBe("0.3000");
  });

  it("s'écrit au centime, dans la langue de l'écran", () => {
    expect(formatPrix("20.9584", "EUR", "fr")).toBe("20,96 €");
    expect(formatPrix("20.9584", "EUR", "en")).toBe("€20.96");
    expect(formatPrix("11.8333", "EUR", "fr")).toBe("11,83 €");
    expect(formatPrix("1234.5", "EUR", "fr")).toBe("1 234,50 €");
  });
});

describe("le devis", () => {
  it("expire à sa date et ne se réutilise pas après", () => {
    const d = "2026-10-06T01:37:58.083165Z";
    expect(devisExpire(d, new Date("2026-10-06T01:37:00Z"))).toBe(false);
    expect(devisExpire(d, new Date("2026-10-06T01:38:00Z"))).toBe(true);
    expect(devisExpire("demain", new Date("2026-10-01T00:00:00Z"))).toBe(true);
  });
});

describe("les états", () => {
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

describe("la confirmation", () => {
  it("dit le prix et le mode en toutes lettres", () => {
    setLangue("fr");
    expect(phraseConfirmation("21,03 €", "Sandbox")).toBe(
      "Commander pour 21,03 € TTC, en sandbox : gratuit, rien ne sera imprimé.",
    );
    expect(phraseConfirmation("21,03 €", "Reel")).toBe(
      "Commander pour 21,03 € TTC, en réel : débité sur votre compte Cloudprinter.",
    );
  });

  it("dit le 409 avec la phrase du moteur", () => {
    setLangue("fr");
    expect(phraseErreur(new Error("trop_tard"))).toBe(
      "La production a commencé, l’annulation n’est plus possible.",
    );
    expect(phraseErreur("devis_expire")).toContain("expiré");
    expect(phraseErreur(new Error("Cloudprinter injoignable"))).toContain("injoignable");
  });
});
