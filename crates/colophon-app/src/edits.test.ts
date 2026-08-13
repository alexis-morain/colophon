// The edit operations are the only code that rewrites an album; they must
// never produce a spread whose template cannot hold its photos, and they
// must leave the input untouched (the undo stack stores references).

import { describe, expect, it } from "vitest";
import { Album, Slot, Spread, templateCapacity } from "./album";
import {
  changeTemplate,
  movePhoto,
  moveBlocker,
  removePhoto,
  rescuePhoto,
  swapPhotos,
  templateChoices,
  triEntries,
} from "./edits";

function slot(n: number): Slot {
  return { src: `p${n}.jpg`, focal: [0.5, 0.5] };
}

function spread(template: string, count: number): Spread {
  return {
    template,
    slots: Array.from({ length: count }, (_, i) => slot(i)),
  };
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

/** Every spread must exactly fill its template: holes never render. */
function assertSound(a: Album) {
  for (const s of a.spreads) {
    expect(s.slots.length).toBe(templateCapacity(s.template));
  }
}

describe("removePhoto", () => {
  it("walks an octo down to a six, dropping the tail photo too", () => {
    const a = album(spread("octo", 8));
    const b = removePhoto(a, 0, 2);
    expect(b.spreads[0].template).toBe("six");
    expect(b.spreads[0].slots.map((s) => s.src)).toEqual([
      "p0.jpg", "p1.jpg", "p3.jpg", "p4.jpg", "p5.jpg", "p6.jpg",
    ]);
    assertSound(b);
  });

  it("keeps the verso side when the family has one", () => {
    const b = removePhoto(album(spread("six_verso", 6), spread("quad", 4)), 1, 0);
    expect(b.spreads[1].template).toBe("trio");
    const c = removePhoto(album(spread("quad", 4)), 0, 0);
    expect(c.spreads[0].template).toBe("trio");
  });

  it("deletes an emptied spread", () => {
    const a = album(spread("solo", 1), spread("duo", 2));
    const b = removePhoto(a, 0, 0);
    expect(b.spreads).toHaveLength(1);
    expect(b.spreads[0].template).toBe("duo");
  });

  it("never mutates its input", () => {
    const a = album(spread("quad", 4));
    const before = JSON.stringify(a);
    removePhoto(a, 0, 1);
    expect(JSON.stringify(a)).toBe(before);
  });
});

describe("changeTemplate", () => {
  it("truncates when the target holds fewer photos", () => {
    const b = changeTemplate(album(spread("quad", 4)), 0, "duo");
    expect(b.spreads[0].slots).toHaveLength(2);
    assertSound(b);
  });

  it("refuses a template the photos cannot fill", () => {
    const a = album(spread("duo", 2));
    expect(changeTemplate(a, 0, "quad")).toBe(a);
  });

  it("only offers templates the count can fill", () => {
    for (const [t, cap] of templateChoices(spread("trio", 3))) {
      expect(t === "trio" || cap <= 3).toBe(true);
    }
  });
});

describe("swapPhotos", () => {
  it("swaps sources and focals together", () => {
    const a = album(spread("duo", 2));
    a.spreads[0].slots[1].focal = [0.1, 0.9];
    const b = swapPhotos(a, 0, 0, 1);
    expect(b.spreads[0].slots[0].src).toBe("p1.jpg");
    expect(b.spreads[0].slots[0].focal).toEqual([0.1, 0.9]);
    expect(b.spreads[0].slots[1].src).toBe("p0.jpg");
  });
});

describe("rescuePhoto", () => {
  const back = { src: "back.jpg", focal: [0.5, 0.42] as [number, number] };

  it("lands on the anchor spread when it can grow", () => {
    const a = album(spread("trio", 3), spread("duo", 2));
    const r = rescuePhoto(a, back, 0);
    expect(r?.at).toBe(0);
    expect(r?.album.spreads[0].template).toBe("quad");
    expect(r?.album.spreads[0].slots[3].src).toBe("back.jpg");
    r && assertSound(r.album);
  });

  it("spills to a neighbour when the anchor is full", () => {
    const a = album(spread("quad", 4), spread("duo", 2));
    const r = rescuePhoto(a, back, 0);
    expect(r?.at).toBe(1);
    expect(r?.album.spreads[1].template).toBe("trio");
    r && assertSound(r.album);
  });

  it("gives up when the whole neighbourhood is full", () => {
    const a = album(spread("quad", 4), spread("quad", 4), spread("quad", 4));
    expect(rescuePhoto(a, back, 1)).toBeNull();
  });
});

describe("triEntries", () => {
  it("lists discards not shown, plus hand-removed photos", () => {
    const a = album(spread("duo", 2)); // shows p0, p1
    const curation = [
      { src: "gone.jpg", reason: "doublon", kept: "p0.jpg", focal: [0.5, 0.42] as [number, number] },
      { src: "p1.jpg", reason: "hors_budget", focal: [0.5, 0.42] as [number, number] },
    ];
    // p9 was shown at build time (thumb indexed) but is in no spread now
    const thumbs = ["p0.jpg", "p1.jpg", "gone.jpg", "p9.jpg"];
    const entries = triEntries(a, curation, thumbs);
    expect(entries.map((e) => [e.src, e.reason])).toEqual([
      ["gone.jpg", "doublon"],
      ["p9.jpg", "retiree"],
    ]);
  });
});

describe("movePhoto", () => {
  it("moves onto a growable neighbour, both templates follow", () => {
    const a = album(spread("quad", 4), spread("trio", 3));
    const b = movePhoto(a, 0, 0, 1);
    expect(b.spreads[0].template).toBe("trio");
    expect(b.spreads[1].template).toBe("quad");
    expect(b.spreads[1].slots[3].src).toBe("p0.jpg");
    assertSound(b);
  });

  it("refuses when the target has no larger exact template", () => {
    const a = album(spread("duo", 2), spread("quad", 4));
    expect(moveBlocker(a, 0, 0, 1)).toBe("target_full");
    expect(movePhoto(a, 0, 0, 1)).toBe(a);
  });

  it("refuses when the source would sacrifice a bystander", () => {
    // a six losing one photo falls to quad: two photos gone for one move
    const a = album(spread("six", 6), spread("duo", 2));
    expect(moveBlocker(a, 0, 0, 1)).toBe("source_breaks");
    expect(movePhoto(a, 0, 0, 1)).toBe(a);
  });

  it("deletes the source spread when the move empties it", () => {
    const a = album(spread("solo", 1), spread("duo", 2));
    const b = movePhoto(a, 0, 0, 1);
    expect(b.spreads).toHaveLength(1);
    expect(b.spreads[0].template).toBe("trio");
    assertSound(b);
  });
});
