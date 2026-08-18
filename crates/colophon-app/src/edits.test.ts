// The edit operations are the only code that rewrites an album; they must
// never produce a spread whose template cannot hold its photos, and they
// must leave the input untouched (the undo stack stores references).

import { describe, expect, it } from "vitest";
import { Album, Slot, Spread, templateCapacity } from "./album";
import {
  changeTemplate,
  duplicateSpread,
  insertSpread,
  movePhoto,
  moveBlocker,
  moveSpread,
  placePhoto,
  removePhoto,
  removeSpread,
  hasGarde,
  renameAlbum,
  rescuePhoto,
  restoreSpread,
  setSlotCaption,
  setSlotCrop,
  setCover,
  setGarde,
  setSpreadText,
  swapPhotos,
  templateChoices,
  toggleLock,
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

describe("the hand-edit badge", () => {
  it("stamps every editing operation, and only them", () => {
    const a = album(spread("quad", 4), spread("duo", 2));
    expect(removePhoto(a, 0, 0).spreads[0].edited).toBe(true);
    expect(changeTemplate(a, 0, "trio").spreads[0].edited).toBe(true);
    expect(swapPhotos(a, 0, 0, 1).spreads[0].edited).toBe(true);
    const moved = movePhoto(a, 0, 0, 1);
    expect(moved.spreads[0].edited).toBe(true);
    expect(moved.spreads[1].edited).toBe(true);
    // locking pins without pretending the spread was edited
    const locked = toggleLock(a, 0);
    expect(locked.spreads[0].locked).toBe(true);
    expect(locked.spreads[0].edited).toBeUndefined();
    expect(toggleLock(locked, 0).spreads[0].locked).toBe(false);
  });
});

describe("setSlotCrop", () => {
  it("clamps focal to [0,1] and zoom to its bounds", () => {
    const a = album(spread("duo", 2));
    const b = setSlotCrop(a, 0, 0, [-0.2, 1.4], 9);
    expect(b.spreads[0].slots[0].focal).toEqual([0, 1]);
    expect(b.spreads[0].slots[0].zoom).toBe(4);
    expect(b.spreads[0].edited).toBe(true);
    // below-fill zoom clamps back to 1, which is the current value: no-op
    expect(setSlotCrop(a, 0, 0, [0.5, 0.5], 0.3)).toBe(a);
  });

  it("is a no-op when nothing changes", () => {
    const a = album(spread("duo", 2));
    expect(setSlotCrop(a, 0, 0, [0.5, 0.5], 1)).toBe(a);
  });
});

describe("captions and text", () => {
  it("sets and clears a photo caption", () => {
    const a = album(spread("duo", 2));
    const b = setSlotCaption(a, 0, 1, "  la plage  ");
    expect(b.spreads[0].slots[1].caption).toBe("la plage");
    expect(b.spreads[0].edited).toBe(true);
    const c = setSlotCaption(b, 0, 1, "   ");
    expect(c.spreads[0].slots[1].caption).toBeUndefined();
  });

  it("writes the free text of a texte spread", () => {
    const a = album(spread("duo", 2));
    const b = insertSpread(a, 0, "texte");
    expect(b.spreads[1].template).toBe("texte");
    expect(b.spreads[1].slots).toHaveLength(0);
    const c = setSpreadText(b, 1, "Un été.\nDeux lignes.");
    expect(c.spreads[1].text).toBe("Un été.\nDeux lignes.");
    assertSound(c);
  });
});

describe("spread manipulation", () => {
  it("moves a spread and stamps it edited", () => {
    const a = album(spread("solo", 1), spread("duo", 2), spread("trio", 3));
    const b = moveSpread(a, 0, 2);
    expect(b.spreads.map((s) => s.template)).toEqual(["duo", "trio", "solo"]);
    expect(b.spreads[2].edited).toBe(true);
  });

  it("duplicates right after itself, without the lock", () => {
    const a = album(spread("duo", 2));
    a.spreads[0].locked = true;
    const b = duplicateSpread(a, 0);
    expect(b.spreads).toHaveLength(2);
    expect(b.spreads[1].template).toBe("duo");
    expect(b.spreads[1].locked).toBe(false);
    expect(b.spreads[1].edited).toBe(true);
  });

  it("inserts a breathing page and removes it again", () => {
    const a = album(spread("duo", 2));
    const b = insertSpread(a, 0, "vide");
    expect(b.spreads[1].template).toBe("vide");
    expect(removeSpread(b, 1).spreads).toHaveLength(1);
  });
});

describe("placePhoto", () => {
  it("replaces the case's photo, keeping the drawer photo's focal", () => {
    const a = album(spread("duo", 2));
    const b = placePhoto(a, 0, 1, { src: "new.jpg", focal: [0.3, 0.6] });
    expect(b.spreads[0].slots[1].src).toBe("new.jpg");
    expect(b.spreads[0].slots[1].focal).toEqual([0.3, 0.6]);
    expect(b.spreads[0].edited).toBe(true);
    assertSound(b);
  });

  it("refuses a photo already on the spread", () => {
    const a = album(spread("duo", 2));
    expect(placePhoto(a, 0, 1, { src: "p0.jpg", focal: [0.5, 0.5] })).toBe(a);
  });
});

describe("restoreSpread", () => {
  it("gives back the composer's spread, badge and lock dropped", () => {
    const origin = spread("duo", 2);
    const abimee: Spread = {
      ...spread("solo", 1),
      edited: true,
      locked: true,
      caption: "ma légende",
    };
    const a = album(abimee);
    const b = restoreSpread(a, 0, origin);
    expect(b.spreads[0].template).toBe("duo");
    expect(b.spreads[0].slots).toHaveLength(2);
    expect(b.spreads[0].edited).toBe(false);
    expect(b.spreads[0].locked).toBe(false);
    expect(b.spreads[0].caption).toBeUndefined();
    // The undo stack holds references: the input never moves.
    expect(a.spreads[0].template).toBe("solo");
    assertSound(b);
  });

  it("leaves an album alone when the index is past the end", () => {
    const a = album(spread("duo", 2));
    expect(restoreSpread(a, 4, spread("solo", 1))).toBe(a);
  });
});

describe("moveBlocker", () => {
  it("refuses a page of text: a photo landing there would swallow it", () => {
    const a = album(spread("duo", 2), spread("solo", 1));
    const avec = {
      ...a,
      spreads: [
        ...a.spreads,
        { template: "texte", slots: [], text: "Le premier soir" },
      ],
    };
    expect(moveBlocker(avec, 0, 0, 2)).toBe("target_text");
    // The breathing page has nothing to lose: dropping a photo on it is what
    // it is for, and it becomes a solo.
    const vide = {
      ...a,
      spreads: [...a.spreads, { template: "vide", slots: [] }],
    };
    expect(moveBlocker(vide, 0, 0, 2)).toBeNull();
    expect(movePhoto(vide, 0, 0, 2).spreads[2].template).toBe("solo");
  });
});

describe("setGarde", () => {
  const garde: Spread = {
    template: "garde",
    slots: [],
    text: "Corse 2013\n\nDu 21 au 29 octobre 2013\nCalvi",
  };

  it("opens the book and nowhere else", () => {
    const a = album(spread("duo", 2), spread("trio", 3));
    const b = setGarde(a, garde);
    expect(b.spreads[0].template).toBe("garde");
    expect(b.spreads).toHaveLength(3);
    expect(hasGarde(b)).toBe(true);
    // Twice over is still once: the page is put back, not stacked.
    expect(setGarde(b, garde).spreads).toHaveLength(3);
  });

  it("takes it away without touching anything else", () => {
    const a = setGarde(album(spread("duo", 2)), garde);
    const b = setGarde(a, null);
    expect(hasGarde(b)).toBe(false);
    expect(b.spreads).toHaveLength(1);
    // An album that never had one is returned as it stands.
    expect(setGarde(b, null)).toBe(b);
  });

  it("follows a rename, on the title line and nowhere else", () => {
    const a = setGarde(album(spread("duo", 2)), garde);
    const b = renameAlbum(a, "Corse, novembre 2013");
    expect(b.spreads[0].text).toBe(
      "Corse, novembre 2013\n\nDu 21 au 29 octobre 2013\nCalvi",
    );
    // And the album it was renamed from is untouched: the undo stack holds it.
    expect(a.spreads[0].text).toBe(garde.text);
  });

  it("prints the cover's title when the cover has one of its own", () => {
    const a = setGarde(album(spread("duo", 2)), garde);
    const b = setCover(a, { title: "Un été", subtitle: "octobre 2013" });
    expect(b.spreads[0].text).toBe(
      "Un été\n\nDu 21 au 29 octobre 2013\nCalvi",
    );
    // Renaming the album no longer moves that line: the book is called what
    // its cover says, and the cover was given a name of its own.
    expect(renameAlbum(b, "Corse 2013").spreads[0].text).toBe(b.spreads[0].text);
    // A cover left blank hands the page back to the album's name.
    expect(setCover(b, { title: "  " }).spreads[0].text).toBe(
      "test\n\nDu 21 au 29 octobre 2013\nCalvi",
    );
  });
});

describe("renameAlbum", () => {
  it("drags along a cover that never had a title of its own", () => {
    const a = { ...album(spread("duo", 2)), cover: { title: "test" } };
    const b = renameAlbum(a, "  Corse 2013 ");
    expect(b.title).toBe("Corse 2013");
    expect(b.cover?.title).toBe("Corse 2013");
  });

  it("leaves a cover its own title", () => {
    const a = {
      ...album(spread("duo", 2)),
      cover: { title: "Un été", subtitle: "juillet" },
    };
    const b = renameAlbum(a, "Corse 2013");
    expect(b.title).toBe("Corse 2013");
    expect(b.cover?.title).toBe("Un été");
  });

  it("refuses an empty name rather than leaving the book nameless", () => {
    const a = album(spread("duo", 2));
    expect(renameAlbum(a, "   ")).toBe(a);
    expect(renameAlbum(a, "test")).toBe(a);
  });
});
