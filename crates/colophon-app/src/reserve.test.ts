// Choisir une candidate de la réserve change l'album par `addPhoto` ou
// `placePhoto`, et rien d'autre ; une planche pleine se dit avant le choix
// et s'ouvre sur la planche d'après.

import { describe, expect, it } from "vitest";
import { Album, Slot, Spread, templateCapacity } from "./album";
import { Candidat } from "./bridge";
import fixture from "./geometrie.fixture.json";
import { Dump, setGeometrie } from "./geometrie";
import { effetDeDepot, pleineAvant, poser, poserApres, TYPE_PHOTO } from "./reserve";

setGeometrie(fixture as unknown as Dump);

function slot(n: number): Slot {
  return { src: `p${n}.jpg`, focal: [0.5, 0.5] };
}

function spread(template: string, count: number): Spread {
  return { template, slots: Array.from({ length: count }, (_, i) => slot(i)) };
}

function album(...spreads: Spread[]): Album {
  return {
    version: 1,
    title: "test",
    root: "/tmp",
    trim_mm: { w: 210, h: 210 },
    bleed_mm: 3,
    spreads,
  };
}

const candidate: Candidat = {
  src: "reserve.jpg",
  score: 6.2,
  taken: "2013-10-27T17:43:10",
  raison: "meme_jour",
  focal: [0.3, 0.2],
};

describe("poser", () => {
  it("ajouter fait grandir la planche, le focal du moteur avec", () => {
    const a = album(spread("duo", 2));
    const r = poser(a, 0, { type: "ajouter" }, candidate);
    if (r.type !== "posee") throw new Error(r.type);
    expect(r.at).toBe(0);
    expect(r.album.spreads[0].template).toBe("trio");
    expect(r.album.spreads[0].slots[2]).toEqual({ src: "reserve.jpg", focal: [0.3, 0.2] });
    expect(r.album.spreads[0].slots.length).toBe(templateCapacity("trio"));
  });

  it("remplacer est placePhoto : la case change, la planche ne grandit pas", () => {
    const a = album(spread("duo", 2));
    const r = poser(a, 0, { type: "remplacer", cell: 1 }, candidate);
    if (r.type !== "remplacee") throw new Error(r.type);
    expect(r.album.spreads[0].template).toBe("duo");
    expect(r.album.spreads[0].slots[1].src).toBe("reserve.jpg");
    expect(r.album.spreads[0].slots[0].src).toBe("p0.jpg");
  });

  it("une planche pleine ne bouge pas et le dit", () => {
    const a = album(spread("quad", 4));
    expect(poser(a, 0, { type: "ajouter" }, candidate)).toEqual({
      type: "pleine",
      raison: "target_full",
    });
    expect(pleineAvant(a, 0)).toBe("target_full");
  });

  it("une page de texte refuse une photo, avant même le choix", () => {
    const a = album({ ...spread("texte", 0), text: "un mot" });
    expect(pleineAvant(a, 0)).toBe("target_text");
    expect(poser(a, 0, { type: "ajouter" }, candidate).type).toBe("pleine");
  });

  it("une planche qui a de la place ne se dit pas pleine", () => {
    expect(pleineAvant(album(spread("duo", 2)), 0)).toBeNull();
  });

  it("remplacer par une photo déjà sur la planche se refuse", () => {
    const a = album(spread("duo", 2));
    const deja = { ...candidate, src: "p0.jpg" };
    expect(poser(a, 0, { type: "remplacer", cell: 1 }, deja)).toEqual({ type: "refus" });
  });
});

describe("poserApres", () => {
  it("insère une planche vide après et y pose la candidate, seule", () => {
    const a = album(spread("quad", 4), spread("duo", 2));
    const r = poserApres(a, 0, candidate);
    expect(r.at).toBe(1);
    expect(r.album.spreads).toHaveLength(3);
    expect(r.album.spreads[1].template).toBe("solo");
    expect(r.album.spreads[1].slots).toEqual([{ src: "reserve.jpg", focal: [0.3, 0.2] }]);
    expect(r.album.spreads[2]).toBe(a.spreads[1]);
  });
});

describe("effetDeDepot", () => {
  // Le tiroir n'autorise que la copie : une case qui annonce « move » à une
  // photo du tiroir fait annuler le dépôt par le navigateur, `drop` ne part
  // jamais (WebKit comme Chromium).
  it("rend copy pour une photo du tiroir", () => {
    expect(effetDeDepot([TYPE_PHOTO])).toBe("copy");
    expect(effetDeDepot(["text/plain", TYPE_PHOTO])).toBe("copy");
  });

  it("garde move pour l'échange de deux cases", () => {
    expect(effetDeDepot(["text/colophon-slot"])).toBe("move");
    expect(effetDeDepot([])).toBe("move");
  });
});
