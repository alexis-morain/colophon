// La veille est un réglage, donc elle se teste comme les deux autres : ce
// qu'elle vaut sans rien en mémoire, ce qu'elle relit, et ce qu'elle fait
// quand le stockage refuse. Le dernier cas n'est pas théorique : une app
// ouverte dans une fenêtre privée, ou un `localStorage` désactivé, doit
// continuer à composer un album, et le pire que ça puisse coûter est
// l'oubli du choix à la fermeture.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type Faux = {
  getItem: (c: string) => string | null;
  setItem: (c: string, v: string) => void;
};

/** Pose un `localStorage` de fortune, ou le retire quand on passe `null`. */
function poser(faux: Faux | null): void {
  if (faux === null) {
    delete (globalThis as { localStorage?: unknown }).localStorage;
    return;
  }
  (globalThis as { localStorage?: unknown }).localStorage = faux;
}

function memoire(depart: Record<string, string> = {}): Faux {
  const sac = { ...depart };
  return {
    getItem: (c) => sac[c] ?? null,
    setItem: (c, v) => {
      sac[c] = v;
    },
  };
}

beforeEach(() => vi.resetModules());
afterEach(() => poser(null));

describe("la veille", () => {
  it("vérifie au lancement tant que personne n'a dit le contraire", async () => {
    poser(memoire());
    const { veille } = await import("./maj");
    expect(veille()).toBe("au-lancement");
  });

  it("vaut au lancement quand il n'y a aucun stockage du tout", async () => {
    poser(null);
    const { veille, setVeille } = await import("./maj");
    expect(veille()).toBe("au-lancement");
    expect(() => setVeille("jamais")).not.toThrow();
    expect(veille()).toBe("jamais");
  });

  it("relit le choix rangé au lancement précédent", async () => {
    poser(memoire({ "colophon.maj": "jamais" }));
    const { veille } = await import("./maj");
    expect(veille()).toBe("jamais");
  });

  it("ignore une valeur que personne n'a écrite", async () => {
    poser(memoire({ "colophon.maj": "peut-être" }));
    const { veille } = await import("./maj");
    expect(veille()).toBe("au-lancement");
  });

  it("range le choix sous sa clé", async () => {
    const sac = memoire();
    poser(sac);
    const { setVeille } = await import("./maj");
    setVeille("jamais");
    expect(sac.getItem("colophon.maj")).toBe("jamais");
    setVeille("au-lancement");
    expect(sac.getItem("colophon.maj")).toBe("au-lancement");
  });

  it("garde le choix en mémoire quand le stockage refuse d'écrire", async () => {
    poser({
      getItem: () => null,
      setItem: () => {
        throw new Error("stockage bloqué");
      },
    });
    const { setVeille, veille } = await import("./maj");
    expect(() => setVeille("jamais")).not.toThrow();
    expect(veille()).toBe("jamais");
  });

  it("vérifie au lancement quand le stockage refuse même de lire", async () => {
    poser({
      getItem: () => {
        throw new Error("stockage bloqué");
      },
      setItem: () => {},
    });
    const { veille } = await import("./maj");
    expect(veille()).toBe("au-lancement");
  });

  it("prévient ses abonnés, et seulement quand la valeur bouge", async () => {
    poser(memoire());
    const { setVeille, abonneVeille } = await import("./maj");
    let appels = 0;
    const desabonne = abonneVeille(() => appels++);
    setVeille("au-lancement");
    expect(appels).toBe(0);
    setVeille("jamais");
    expect(appels).toBe(1);
    setVeille("jamais");
    expect(appels).toBe(1);
    desabonne();
    setVeille("au-lancement");
    expect(appels).toBe(1);
  });
});
