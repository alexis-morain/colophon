// La fiche d'une photo formate sans DOM : le temps de pose, les
// coordonnées, le poids, le ppi dans sa case. Vitest tourne sans
// `navigator`, langue anglaise par défaut : chaque cas pose la sienne.

import { afterEach, describe, expect, it } from "vitest";
import { setLangue } from "./i18n";
import {
  formatCoordonnees,
  formatNombre,
  formatOctets,
  formatTempsDePose,
  ppiDansSaCase,
} from "./fiche";

afterEach(() => setLangue("en"));

describe("formatTempsDePose", () => {
  it("garde la fraction du moteur et ajoute l'unité", () => {
    setLangue("fr");
    expect(formatTempsDePose("1/250")).toBe("1/250 s");
    expect(formatTempsDePose("30")).toBe("30 s");
  });
  it("met la virgule en français, le point en anglais", () => {
    setLangue("fr");
    expect(formatTempsDePose("2.5")).toBe("2,5 s");
    setLangue("en");
    expect(formatTempsDePose("2.5")).toBe("2.5 s");
  });
});

describe("formatCoordonnees", () => {
  it("quatre décimales, signées, dans la langue", () => {
    setLangue("fr");
    expect(formatCoordonnees(42.5623, 8.741866999924259)).toBe("42,5623, 8,7419");
    setLangue("en");
    expect(formatCoordonnees(-33.8688, 151.2093)).toBe("-33.8688, 151.2093");
  });
});

describe("formatOctets", () => {
  it("octets, kilo, méga, giga, avec l'unité de la langue", () => {
    setLangue("fr");
    expect(formatOctets(512)).toBe("512 o");
    expect(formatOctets(48_000)).toBe("48 Ko");
    expect(formatOctets(1_915_517)).toBe("1,9 Mo");
    expect(formatOctets(2_400_000_000)).toBe("2,40 Go");
    setLangue("en");
    expect(formatOctets(1_915_517)).toBe("1.9 MB");
    expect(formatOctets(512)).toBe("512 B");
  });
});

describe("formatNombre", () => {
  it("coupe les zéros inutiles jusqu'au nombre de décimales demandé", () => {
    setLangue("fr");
    expect(formatNombre(4.12, 2)).toBe("4,12");
    expect(formatNombre(35, 2)).toBe("35");
    expect(formatNombre(2.4, 1)).toBe("2,4");
  });
});

describe("ppiDansSaCase", () => {
  it("la règle du prévol sur les pixels de la fiche, arrondie", () => {
    // 3264 px sur 200 mm : 25,4 / (200 / 3264) ≈ 414.
    expect(ppiDansSaCase({ largeur: 3264, hauteur: 2448 }, { w: 200, h: 150 }, 1)).toBe(415);
    // Le zoom manuel recadre dans les mêmes pixels.
    expect(ppiDansSaCase({ largeur: 3264, hauteur: 2448 }, { w: 200, h: 150 }, 2)).toBe(207);
  });
  it("sans case, rien : la fiche ne s'ouvre pas que depuis une case", () => {
    expect(ppiDansSaCase({ largeur: 3264, hauteur: 2448 }, null, 1)).toBeNull();
    expect(ppiDansSaCase({ largeur: 0, hauteur: 0 }, { w: 200, h: 150 }, 1)).toBeNull();
  });
});
