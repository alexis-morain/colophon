// La commande, sans DOM : quand « Commander » s'affiche, le prix, les états
// et la confirmation. Vitest tourne en anglais par défaut : chaque cas pose
// sa langue.

import { afterEach, describe, expect, it } from "vitest";
import type { Preparation, PrevolReport } from "./bridge";
import {
  annulable,
  commandePossible,
  formatPrix,
  phraseConfirmation,
  phraseErreur,
  phraseEtat,
} from "./commande";
import { setLangue } from "./i18n";

afterEach(() => setLangue("en"));

const preparation = (ok: boolean): Preparation => ({
  dossier: "Album – Cloudprinter",
  fichiers: ["album-print.pdf", "album-cover.pdf"],
  rapport: { ok, bloquants: ok ? 0 : 1, defauts: [] } as unknown as PrevolReport,
});

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
