// L'état du dossier de photos d'un album ouvert, tel que l'écran le dit.
// Trois états et pas deux : sous le bac à sable du Mac App Store, un dossier
// présent mais fermé à l'app (« à revoir ») n'appelle pas la même phrase
// qu'un dossier disparu, et c'est lui qui a un bouton.

import { afterEach, describe, expect, it } from "vitest";
import { etatRacine, refusRevoir } from "./racine";
import { setLangue, t } from "./i18n";

const ouvert = (root_present: boolean, dossier_a_revoir: boolean) => ({
  root_present,
  dossier_a_revoir,
});

afterEach(() => setLangue("en"));

describe("l'état du dossier de photos", () => {
  it("est présent quand tout se lit", () => {
    expect(etatRacine(ouvert(true, false))).toBe("present");
  });

  it("est introuvable quand le dossier a disparu", () => {
    expect(etatRacine(ouvert(false, false))).toBe("introuvable");
  });

  it("est à revoir quand le bac à sable le ferme, qu'il existe ou non", () => {
    expect(etatRacine(ouvert(true, true))).toBe("a-revoir");
    expect(etatRacine(ouvert(false, true))).toBe("a-revoir");
  });

  it("dit pourquoi un dossier rechoisi est refusé, dans la langue de l'interface", () => {
    setLangue("fr");
    expect(refusRevoir("refus-revoir:autre-nom:corse-2013")).toBe(
      "Ce n’est pas le dossier de cet album : il s’appelait « corse-2013 ».",
    );
    expect(refusRevoir("refus-revoir:manquantes:2:3")).toBe(
      "Ce dossier ne porte pas les photos de l’album : 2 sur 3 manquent.",
    );
    setLangue("en");
    expect(refusRevoir("refus-revoir:autre-nom:corse-2013")).toContain("“corse-2013”");
    expect(refusRevoir("refus-revoir:manquantes:2:3")).toContain("2 of 3");
    // Un nom de dossier peut porter des deux-points.
    expect(refusRevoir("Error: refus-revoir:autre-nom:a:b")).toContain("“a:b”");
    // Tout le reste garde la phrase générique.
    expect(refusRevoir("marque-page refusé par le système")).toBeNull();
  });

  it("le dit dans les deux langues, avec un bouton qui nomme le geste", () => {
    setLangue("fr");
    expect(t("racine.a.revoir")).toContain("Colophon a besoin de revoir le dossier de photos");
    expect(t("racine.a.revoir.bouton")).not.toBe("");
    setLangue("en");
    expect(t("racine.a.revoir")).toContain("photo folder");
    expect(t("racine.a.revoir.bouton")).not.toBe("");
  });
});
