// Les deux dictionnaires doivent rester en face l'un de l'autre. Une clé
// ajoutée d'un côté et oubliée de l'autre ne se voit pas à l'écran tant que
// personne n'ouvre l'application dans l'autre langue, c'est-à-dire jamais
// pendant le développement.

import { describe, expect, it } from "vitest";
import { EN, FR, setLangue, t } from "./i18n";

describe("dictionnaires", () => {
  it("ont exactement les mêmes clés", () => {
    const fr = Object.keys(FR).sort();
    const en = Object.keys(EN).sort();
    expect(en).toEqual(fr);
  });

  it("n'ont aucune entrée vide", () => {
    for (const [cle, texte] of Object.entries({ ...FR, ...EN })) {
      expect(texte.trim(), cle).not.toBe("");
    }
  });

  /** Les trous d'un texte sont un contrat : `{poids}` traduit en `{weight}`
   *  laisse un trou béant à l'écran, et personne ne le voit avant la sortie. */
  it("ont les mêmes trous dans les mêmes clés", () => {
    const trous = (s: string) => (s.match(/\{(\w+)\}/g) ?? []).sort();
    for (const cle of Object.keys(FR) as (keyof typeof FR)[]) {
      expect(trous(EN[cle]), cle).toEqual(trous(FR[cle]));
    }
  });

  /** Zéro tiret cadratin, dans les deux langues : la règle vaut pour tout
   *  texte qui sort, et l'anglais est le premier endroit où il revient. */
  it("ne contiennent aucun tiret cadratin", () => {
    for (const [cle, texte] of Object.entries({ ...FR, ...EN })) {
      expect(texte.includes("—"), cle).toBe(false);
    }
  });
});

describe("t", () => {
  it("remplit les trous et laisse les autres tels quels", () => {
    setLangue("fr");
    expect(t("stockage.supprime", { titre: "Corse", poids: "183 Mo" })).toBe(
      "« Corse » supprimé, 183 Mo libérés.",
    );
    expect(t("stockage.planches", {})).toBe("{n} planches");
  });

  it("suit la langue choisie", () => {
    setLangue("en");
    expect(t("bar.enregistrer")).toBe("Save");
    setLangue("fr");
    expect(t("bar.enregistrer")).toBe("Enregistrer");
  });
});
