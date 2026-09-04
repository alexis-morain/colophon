// The scene's own arithmetic, without a canvas and without the engine.
//
// The exhaustive comparison lives in `parity.test.ts`: nine spreads, six
// page shapes, against the engine's golden dump. What is tested here is what
// the fixture cannot show — the hit test that replaces the `<div>` a canvas
// takes away, and the two rules the assembler applies on top of the dump.

import { describe, expect, it } from "vitest";
import fixture from "./geometrie.fixture.json";
import { Album, Spread, spreadGeometry } from "./album";
import { Dump, setGeometrie } from "./geometrie";
import { enOrdreDeLecture, nomDObjet } from "./SceneProxies";
import { setLangue } from "./i18n";
import {
  angleEcran,
  avecRecadrage,
  contains,
  corners,
  decalage,
  depthOfCell,
  distanceToTrim,
  hitTest,
  replier,
  SceneObject,
  sceneOf,
  touche,
  traverseLePli,
} from "./scene";
import { Objet } from "./album";

setGeometrie(fixture as unknown as Dump);

const g = spreadGeometry({
  trim_mm: fixture.trim_mm,
  bleed_mm: fixture.bleed_mm,
} as Album);

/** A width proportional to the glyph count: enough to give captions ink,
 *  and no font file in sight. */
const mesure = (s: string, tailleMm: number) => s.length * tailleMm * 0.5;

function planche(template: string, n: number): Spread {
  return {
    template,
    slots: Array.from({ length: n }, (_, i) => ({
      src: `${i}.jpg`,
      focal: [0.5, 0.42] as [number, number],
    })),
  };
}

describe("what sits under a point", () => {
  it("answers nothing for bare paper", () => {
    const scene = sceneOf(planche("duo_portrait", 2), g, mesure);
    // The gutter runs down the middle of a two-cell spread and belongs to
    // no object: the click that lands there deselects, it does not select.
    expect(hitTest(scene, g.w / 2, g.h / 2)).toBeNull();
    // And nothing at all lies outside the media box.
    expect(hitTest(scene, -10, -10)).toBeNull();
  });

  it("answers the photograph under the pointer", () => {
    const scene = sceneOf(planche("duo_portrait", 2), g, mesure);
    const first = scene.objects[0].rect;
    const at = hitTest(scene, first.x + first.w / 2, first.y + first.h / 2);
    expect(at).toBe(0);
    expect(scene.objects[at!].role).toMatchObject({ role: "photo", cell: 0 });
  });

  it("reads from the front: a caption laid over a photograph wins", () => {
    const spread = planche("full1", 1);
    spread.slots[0].caption = "la traversée";
    const scene = sceneOf(spread, g, mesure);
    // A full-bleed photograph covers the whole media box, so its caption —
    // painted after it — sits over it. The reader sees the caption there,
    // and so must the hit test.
    const legende = scene.objects[1];
    expect(legende.role.role).toBe("photo_caption");
    const at = hitTest(scene, legende.rect.x + 1, legende.rect.y + 1);
    expect(at).toBe(1);
  });

  it("is the paint order read backwards, whatever the reading order says", () => {
    const spread = planche("quad", 4);
    spread.caption = "un chapitre";
    const scene = sceneOf(spread, g, mesure);
    // The chapter caption is last painted and last read here; what the hit
    // test uses is the depth, and the depth is the index.
    const dernier = scene.objects.length - 1;
    const r = scene.objects[dernier].rect;
    expect(hitTest(scene, r.x + 0.5, r.y + 0.5)).toBe(dernier);
  });

  it("holds a point on its own edge", () => {
    const r = { x: 10, y: 20, w: 30, h: 40 };
    expect(contains(r, 10, 20)).toBe(true);
    expect(contains(r, 40, 60)).toBe(true);
    expect(contains(r, 40.001, 60)).toBe(false);
  });
});

describe("cells and depths", () => {
  it("translates a cell into a depth, and says so when there is none", () => {
    const scene = sceneOf(planche("quad", 4), g, mesure);
    expect(depthOfCell(scene, 2)).toBe(2);
    expect(depthOfCell(scene, 9)).toBeNull();
  });

  it("keeps the reading rank of a caption behind its own photograph", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "la plage";
    const scene = sceneOf(spread, g, mesure);
    expect(scene.objects.map((o) => o.role.role)).toEqual([
      "photo",
      "photo",
      "photo_caption",
    ]);
    expect(scene.objects.map((o) => o.reading)).toEqual([0, 1, 2]);
  });
});

describe("what the assembler adds on top of the dump", () => {
  it("gives an empty caption no object at all", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "";
    expect(sceneOf(spread, g, mesure).objects).toHaveLength(2);
  });

  it("renders a template nobody knows as one margined box, silently", () => {
    const scene = sceneOf(planche("gabarit-repare-a-la-main", 3), g, mesure);
    expect(scene.objects).toHaveLength(1);
    expect(scene.objects[0].role.role).toBe("photo");
  });

  it("makes the three text pages one role", () => {
    for (const template of ["garde", "texte", "colophon"]) {
      const spread = planche(template, 0);
      spread.text = "Un été\n\nCalvi, Corse";
      const scene = sceneOf(spread, g, mesure);
      expect(scene.objects).toHaveLength(1);
      const role = scene.objects[0].role;
      expect(role.role).toBe("text");
      // The blank line prints nothing and still takes its turn.
      if (role.role === "text") {
        expect(role.lines.map((l) => l.text)).toEqual(["Un été", "Calvi, Corse"]);
        expect(role.lines[1].dyMm).toBeGreaterThan(0);
      }
    }
  });

  it("gives a text page with nothing written no object", () => {
    const spread = planche("texte", 0);
    spread.text = "";
    expect(sceneOf(spread, g, mesure).objects).toHaveLength(0);
  });
});

// The names the keyboard says out loud. They are built from the role code
// and its parameters — never from a sentence the engine wrote — so the two
// languages each get their own word order instead of one inheriting the
// other's.
describe("what the keyboard calls an object", () => {
  it("names a photograph by its rank and its file", () => {
    setLangue("fr");
    const scene = sceneOf(planche("duo", 2), g, mesure);
    expect(nomDObjet(scene.objects[0], scene)).toBe("Photo 1 sur 2, 0.jpg");
    setLangue("en");
    expect(nomDObjet(scene.objects[1], scene)).toBe("Photo 2 of 2, 1.jpg");
  });

  it("says a folder out loud to nobody", () => {
    setLangue("fr");
    const spread = planche("solo", 1);
    spread.slots[0].src = "vacances/corse/IMG_0421.jpg";
    const scene = sceneOf(spread, g, mesure);
    expect(nomDObjet(scene.objects[0], scene)).toContain("IMG_0421.jpg");
    expect(nomDObjet(scene.objects[0], scene)).not.toContain("vacances");
  });

  it("names a caption after the photograph it belongs to", () => {
    setLangue("fr");
    const spread = planche("duo", 2);
    spread.slots[1].caption = "la plage";
    const scene = sceneOf(spread, g, mesure);
    const legende = scene.objects[scene.objects.length - 1];
    expect(nomDObjet(legende, scene)).toBe("Légende de la photo 2 : la plage");
  });

  it("has something to say about a chapter with no title", () => {
    setLangue("fr");
    const spread = planche("duo", 2);
    spread.caption = "";
    const scene = sceneOf(spread, g, mesure);
    const chapitre = scene.objects[scene.objects.length - 1];
    expect(nomDObjet(chapitre, scene)).toBe("Titre de chapitre, vide");
  });

  it("names a block of text by its first line, whichever page it is", () => {
    setLangue("fr");
    for (const template of ["garde", "texte", "colophon"]) {
      const spread = planche(template, 0);
      spread.text = "Un été & deux hivers\n\nCalvi";
      const scene = sceneOf(spread, g, mesure);
      expect(nomDObjet(scene.objects[0], scene)).toBe(
        "Bloc de texte : Un été & deux hivers",
      );
    }
  });

  // Une légende de photo vide ne porte aucun objet, là où une légende de
  // chapitre vide en porte un : la première n'aurait rien à dire, la seconde
  // est le seul chemin vers le champ qui la nommera.
  it("gives a photograph caption of no text no object at all", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "";
    const scene = sceneOf(spread, g, mesure);
    expect(scene.objects.filter((o) => o.role.role === "photo_caption")).toHaveLength(0);
  });

  it("counts only the photographs a spread actually shows", () => {
    setLangue("fr");
    // Un album réparé à la main porte quatre photos sous un gabarit qui n'en
    // déclare que deux : le moteur en pose deux, la scène aussi, et le nom
    // ne promet pas un total que la planche ne montre pas.
    const scene = sceneOf(planche("duo", 4), g, mesure);
    const photos = scene.objects.filter((o) => o.role.role === "photo");
    expect(photos).toHaveLength(2);
    expect(nomDObjet(photos[1], scene)).toBe("Photo 2 sur 2, 1.jpg");
  });

  it("names the one box an unknown template falls back to", () => {
    setLangue("fr");
    const scene = sceneOf(planche("gabarit-que-personne-ne-connait", 3), g, mesure);
    expect(scene.objects).toHaveLength(1);
    expect(nomDObjet(scene.objects[0], scene)).toBe("Photo 1 sur 1, 0.jpg");
  });

  it("has nothing special to say about a verso", () => {
    setLangue("fr");
    const spread = planche("full1_verso", 1);
    spread.slots[0].caption = "au dos";
    const scene = sceneOf(spread, g, mesure);
    expect(scene.objects.map((o) => o.role.role)).toEqual([
      "photo",
      "photo_caption",
    ]);
    expect(nomDObjet(scene.objects[1], scene)).toBe(
      "Légende de la photo 1 : au dos",
    );
  });
});

// L'ordre où l'application prononce une planche. Il ne se lit nulle part
// ailleurs : le DOM peint dans l'ordre du flux d'impression, et la couche
// d'accessibilité est le seul endroit où ces deux ordres peuvent diverger.
describe("the order a spread is read in", () => {
  it("reads the pictures, then their captions, then what belongs to the spread", () => {
    const spread = planche("quad", 4);
    spread.slots[1].caption = "la plage";
    spread.slots[3].caption = "le port";
    spread.caption = "Calvi";
    const scene = sceneOf(spread, g, mesure);
    expect(enOrdreDeLecture(scene).map(({ o }) => o.role.role)).toEqual([
      "photo",
      "photo",
      "photo",
      "photo",
      "photo_caption",
      "photo_caption",
      "chapter_caption",
    ]);
    // Et une légende se lit après la photo qu'elle nomme, pas après la
    // première venue : les rangs suivent les cases.
    const legendes = enOrdreDeLecture(scene)
      .map(({ o }) => o.role)
      .filter((r) => r.role === "photo_caption");
    expect(legendes.map((r) => (r as { cell: number }).cell)).toEqual([1, 3]);
  });

  it("reads the chapter title after the block of text", () => {
    const spread = planche("texte", 0);
    spread.text = "Première ligne.";
    spread.caption = "Un chapitre";
    const scene = sceneOf(spread, g, mesure);
    expect(enOrdreDeLecture(scene).map(({ o }) => o.role.role)).toEqual([
      "text",
      "chapter_caption",
    ]);
  });

  it("keeps the reading rank a strictly increasing count", () => {
    const spread = planche("quad", 4);
    spread.slots.forEach((s, i) => (s.caption = `légende ${i}`));
    spread.caption = "Calvi";
    const rangs = enOrdreDeLecture(sceneOf(spread, g, mesure)).map(
      ({ o }) => o.reading,
    );
    expect(rangs).toEqual([...rangs].sort((a, b) => a - b));
    expect(new Set(rangs).size).toBe(rangs.length);
  });
});

// A gesture in flight is the same scene with one framing not yet written
// down. Both renderers read it, so it lives with the scene rather than
// inside either of them.
describe("a crop still being made", () => {
  it("reframes one cell and leaves every other object alone", () => {
    const spread = planche("duo", 2);
    spread.slots[0].caption = "la plage";
    const scene = sceneOf(spread, g, mesure);
    const pendant = avecRecadrage(scene, 1, [0.2, 0.8], 2.5);
    expect(pendant.objects[1].role).toMatchObject({
      role: "photo",
      cell: 1,
      focal: [0.2, 0.8],
      zoom: 2.5,
    });
    // Same rectangles, same order, same everything else: a crop moves what
    // shows inside a cell, never the cell.
    expect(pendant.objects.map((o) => o.rect)).toEqual(
      scene.objects.map((o) => o.rect),
    );
    expect(pendant.objects[0].role).toEqual(scene.objects[0].role);
    expect(pendant.objects[2].role).toEqual(scene.objects[2].role);
  });

  it("leaves the scene it was given untouched", () => {
    const scene = sceneOf(planche("duo", 2), g, mesure);
    avecRecadrage(scene, 0, [0, 0], 4);
    expect(scene.objects[0].role).toMatchObject({ focal: [0.5, 0.42], zoom: 1 });
  });
});

// ---- les objets libres, et l'angle qu'ils apportent ---------------------
//
// Ce que la fixture de parité épingle, c'est l'accord avec le moteur. Ce qui
// se teste ici est ce qu'elle ne peut pas montrer : qu'un clic atteint une
// boîte tournée, que la coupe se mesure sur des coins et pas sur un
// rectangle, et que le signe de la rotation est pris une fois pour toutes.

function bloc(o: Partial<Objet> = {}): Objet {
  return {
    x: 20,
    y: 20,
    w: 60,
    h: 20,
    type: "texte",
    texte: "un bloc",
    taille_pt: 11,
    ...o,
  } as Objet;
}

describe("free objects on the scene", () => {
  it("sit on top of everything a template produced", () => {
    const p = planche("duo", 2);
    p.caption = "Corse, 2013";
    p.objets = [bloc(), bloc({ x: 200 })];
    const scene = sceneOf(p, g, mesure);
    const roles = scene.objects.map((o) => o.role.role);
    expect(roles.slice(-2)).toEqual(["free_text", "free_text"]);
    expect(roles.indexOf("free_text")).toBeGreaterThan(
      roles.lastIndexOf("chapter_caption"),
    );
    // Leur index pointe dans `spread.objets`, comme `cell` pointe dans
    // `spread.slots` : c'est par là qu'une modification repart.
    const libres = scene.objects.filter((o) => o.role.role === "free_text");
    expect(libres.map((o) => (o.role as { index: number }).index)).toEqual([0, 1]);
    // Et la lecture continue au-delà du gabarit, sans trou ni doublon.
    expect(scene.objects.map((o) => o.reading)).toEqual(
      scene.objects.map((_, i) => i),
    );
  });

  it("carry the box the reader drew, flipped into the screen's frame", () => {
    const p = planche("duo", 2);
    p.objets = [bloc({ x: 20, y: 20, w: 60, h: 20 })];
    const objets = sceneOf(p, g, mesure).objects;
    const o = objets[objets.length - 1];
    // Le moteur pose l'origine en bas à gauche, l'écran en haut à gauche :
    // c'est la seule conversion que ce fichier fasse, et elle se lit ici.
    expect(o.rect).toEqual({ x: 20, y: g.h - 40, w: 60, h: 20 });
  });

  it("keep the engine's angle unflipped, and hand the flip to a renderer", () => {
    const p = planche("duo", 2);
    p.objets = [bloc({ angle: 30 })];
    const objets = sceneOf(p, g, mesure).objects;
    expect(objets[objets.length - 1].angle).toBe(30);
    // Le sens trigonométrique du moteur est le sens horaire d'un écran : la
    // négation est prise une fois, ici, et trois rendus la lisent.
    expect(angleEcran(30)).toBe(-30);
  });
});

describe("what sits under a point, once boxes can turn", () => {
  it("reaches a turned box where the upright one would miss", () => {
    // Une boîte plate et large, tournée d'un quart de tour : le point qui la
    // manquait droite tombe dedans tournée, et l'inverse.
    const o: SceneObject = {
      rect: { x: 100, y: 100, w: 80, h: 10 },
      angle: 0,
      reading: 0,
      role: { role: "photo", cell: 0, src: "a", focal: [0.5, 0.5], zoom: 1 },
    };
    const dehors = { x: 142, y: 130 };
    const dedans = { x: 175, y: 104 };
    expect(touche(o, dehors.x, dehors.y)).toBe(false);
    expect(touche(o, dedans.x, dedans.y)).toBe(true);

    const tourne = { ...o, angle: 90 };
    expect(touche(tourne, dehors.x, dehors.y)).toBe(true);
    expect(touche(tourne, dedans.x, dedans.y)).toBe(false);
  });

  it("still answers the topmost object, turned or not", () => {
    const p = planche("duo", 2);
    p.objets = [bloc({ x: 20, y: 20, w: 60, h: 20, angle: 20 })];
    const scene = sceneOf(p, g, mesure);
    const libre = scene.objects.length - 1;
    const c = {
      x: scene.objects[libre].rect.x + 30,
      y: scene.objects[libre].rect.y + 10,
    };
    // Le centre d'une boîte est dans la boîte quel que soit l'angle : c'est
    // le point fixe de la rotation.
    expect(hitTest(scene, c.x, c.y)).toBe(libre);
  });
});

describe("the distance to the guillotine, with an angle", () => {
  it("returns the upright numbers untouched at zero degrees", () => {
    // Les coins d'une boîte droite sont ceux du rectangle, au bit près :
    // sans ça, un objet droit mesurerait autre chose qu'avant l'angle.
    const r = { x: 11.2, y: 18.2, w: 87.3, h: 140.6 };
    const c = corners(r, 0);
    expect(c[0]).toEqual({ x: r.x, y: r.y });
    expect(c[2]).toEqual({ x: r.x + r.w, y: r.y + r.h });
    const avant = Math.min(
      r.x - g.bleed,
      r.y - g.bleed,
      g.w - g.bleed - (r.x + r.w),
      g.h - g.bleed - (r.y + r.h),
    );
    expect(distanceToTrim(r, 0, g)).toBe(avant);
  });

  it("sees a corner cross the cut that the upright box cleared", () => {
    const r = { x: 100, y: 4, w: 120, h: 8 };
    expect(distanceToTrim(r, 0, g)).toBeGreaterThan(0);
    expect(distanceToTrim(r, 30, g)).toBeLessThan(0);
  });

  it("stops a turned corner at the fold the upright box cleared", () => {
    const pli = g.w / 2;
    const r = { x: pli - 55, y: 100, w: 50, h: 40 };
    expect(traverseLePli(r, 0, g)).toBe(false);
    expect(traverseLePli(r, 45, g)).toBe(true);
  });
});

describe("a block that wraps to its box", () => {
  it("breaks at words and loses nothing", () => {
    const { lignes, tropLarge } = replier("un deux trois quatre", 30, 4, mesure);
    expect(lignes.length).toBeGreaterThan(1);
    for (const l of lignes) expect(mesure(l, 4)).toBeLessThanOrEqual(30);
    expect(lignes.join(" ")).toBe("un deux trois quatre");
    expect(tropLarge).toBe(false);
  });

  it("reports a word wider than the box instead of cutting it", () => {
    const { lignes, tropLarge } = replier("court anticonstitutionnel", 30, 4, mesure);
    expect(tropLarge).toBe(true);
    expect(lignes).toContain("anticonstitutionnel");
  });

  it("keeps a blank paragraph, because it is spacing", () => {
    const { lignes } = replier("un\n\ntrois", 100, 4, mesure);
    expect(lignes).toEqual(["un", "", "trois"]);
  });

  it("says so when the set text is taller than its box", () => {
    const p = planche("duo", 2);
    p.objets = [bloc({ h: 60, texte: "un\ndeux" }), bloc({ h: 4, texte: "un\ndeux\ntrois" })];
    const [tient, deborde] = sceneOf(p, g, mesure).objects.slice(-2);
    expect((tient.role as { overflow: boolean }).overflow).toBe(false);
    expect((deborde.role as { overflow: boolean }).overflow).toBe(true);
    // Et rien n'a été retiré pour autant : ce qui déborde s'imprime.
    expect((deborde.role as { lines: unknown[] }).lines).toHaveLength(3);
  });

  it("offsets a line by its alignment, once, for every renderer", () => {
    expect(decalage("gauche", 40, 6)).toBe(0);
    expect(decalage("centre", 40, 6)).toBe(17);
    expect(decalage("droite", 40, 6)).toBe(34);
  });
});
