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
  avecRecadrage,
  contains,
  depthOfCell,
  hitTest,
  sceneOf,
} from "./scene";

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
