// The thumbnails a spread needs, as decoded images, and the two things the
// editor says about a photograph once it has one.
//
// `thumbs.ts` caches the *URL* of a thumbnail, which is all an `<img>` tag
// needs. A canvas needs the decoded image itself, and so does anyone who
// wants to know how many pixels a photograph has without reopening the
// original. Hence one cache, shared: the DOM renderer reads its badges from
// the element it already drew, the canvas reads them from here, and the rule
// they read them by is written once.

import { useSyncExternalStore } from "react";
import {
  DARK_MEAN_LUMA,
  MIN_EFFECTIVE_PPI,
  Rect,
  Reglage,
  effectivePpi,
  slidingRoom,
} from "./album";
import { ReleveAlbum, releveAlbum } from "./bridge";
import { t } from "./i18n";
import { appliquer, estIdentite } from "./reglage";
import {
  cachedThumb,
  histoLuma,
  loadThumb,
  meanLuma,
  moyenneCorrigee,
} from "./thumbs";

const images = new Map<string, HTMLImageElement>();
const enCours = new Set<string>();
const abonnes = new Set<(src: string) => void>();

/**
 * The decoded thumbnail of a source, or null while it is on its way. Asking
 * starts the loading; whoever is subscribed hears about it when it lands.
 */
export function imageDe(src: string): HTMLImageElement | null {
  const prete = images.get(src);
  if (prete) return prete;
  if (enCours.has(src) || typeof Image === "undefined") return null;
  enCours.add(src);
  const poser = (url: string) => {
    const img = new Image();
    img.onload = () => {
      enCours.delete(src);
      images.set(src, img);
      abonnes.forEach((f) => f(src));
    };
    img.onerror = () => enCours.delete(src);
    img.src = url;
  };
  const dejaLa = cachedThumb(src);
  if (dejaLa) poser(dejaLa);
  else loadThumb(src).then(poser, () => enCours.delete(src));
  return null;
}

/** One pre-adjusted bitmap per photo — the last committed réglage only, so
 *  the pool stays the size of the thumbnail pool. */
const reglees = new Map<string, { cle: string; bitmap: HTMLCanvasElement }>();

/**
 * The thumbnail of a photo with its committed adjustment burnt in: the
 * canvas renderer's fallback where `ctx.filter` does not exist. Computed
 * lazily at the repaint a commit triggers, cached by (src, réglage), and
 * never during a slider drag — the caller ignores the draft on this path,
 * so the case follows at release, and that is all. The thumbnail cache on
 * disk stays untouched: this adjusts a copy, in memory, for the screen.
 */
export function imageRegleeDe(src: string, r: Reglage): CanvasImageSource | null {
  const img = imageDe(src);
  if (!img) return null;
  const cle = `${r.expo}|${r.contraste}|${r.nb}`;
  const hit = reglees.get(src);
  if (hit && hit.cle === cle) return hit.bitmap;
  const c = document.createElement("canvas");
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  const ctx = c.getContext("2d");
  if (!ctx) return img;
  ctx.drawImage(img, 0, 0);
  const data = ctx.getImageData(0, 0, c.width, c.height);
  appliquer(data.data, r);
  ctx.putImageData(data, 0, 0);
  reglees.set(src, { cle, bitmap: c });
  return c;
}

/** Runs whenever a thumbnail finishes decoding. */
export function surImage(hook: (src: string) => void): () => void {
  abonnes.add(hook);
  return () => {
    abonnes.delete(hook);
  };
}

// ---- the relevé, once per album -----------------------------------------

let releve: ReleveAlbum | null = null;
/** Chaque chargement a son numéro : la réponse d'un album qu'on a quitté
 *  arrive parfois après celle du suivant, et ne doit rien écraser. */
let generation = 0;
let versionReleve = 0;
const abonnesReleve = new Set<() => void>();

function annoncerReleve() {
  versionReleve++;
  abonnesReleve.forEach((f) => f());
}

/**
 * Lire le relevé de l'album qui vient de s'ouvrir, et oublier celui
 * d'avant. Appelé à chaque ouverture : un autre album, d'autres photos.
 * Tant qu'il n'est pas là, ou s'il n'a pas pu se lire, aucune case ne porte
 * d'alerte — l'alerte ne s'invente rien.
 */
export function chargerReleve(
  lire: () => Promise<ReleveAlbum> = releveAlbum,
): Promise<void> {
  const g = ++generation;
  releve = null;
  annoncerReleve();
  return lire().then(
    (r) => {
      if (g !== generation) return;
      releve = r;
      annoncerReleve();
    },
    () => {},
  );
}

export function releveCourant(): ReleveAlbum | null {
  return releve;
}

/** Le relevé de l'album ouvert, et un nouveau rendu quand il arrive. */
export function useReleve(): ReleveAlbum | null {
  useSyncExternalStore(
    (f) => {
      abonnesReleve.add(f);
      return () => abonnesReleve.delete(f);
    },
    () => versionReleve,
    () => versionReleve,
  );
  return releve;
}

// ---- the alerts ------------------------------------------------------------

/** Ce qu'une case dit d'elle-même : trois alertes, un code chacune, dans
 *  cet ordre. Le texte est à `nomDAlerte` et `texteDAlerte`. */
export type Alerte =
  | { code: "sous_resolution"; ppi: number }
  | { code: "sombre" }
  | { code: "floue" };

/** What a case says about itself, over the photograph. */
export type Badges = {
  alertes: Alerte[];
  /** The photograph fills its cell exactly: no gesture can slide it. */
  sansMarge: boolean;
};

/** L'alerte dite en quelques mots, pour le nom d'un objet (VoiceOver). */
export function nomDAlerte(a: Alerte): string {
  switch (a.code) {
    case "sous_resolution":
      return t("alerte.sous_resolution", { ppi: a.ppi, plancher: MIN_EFFECTIVE_PPI });
    case "sombre":
      return t("alerte.sombre");
    case "floue":
      return t("alerte.floue");
  }
}

/** L'alerte avec son remède, pour l'infobulle d'une case. */
export function texteDAlerte(a: Alerte): string {
  switch (a.code) {
    case "sous_resolution":
      return t("planche.warn.ppi", { ppi: a.ppi, plancher: MIN_EFFECTIVE_PPI });
    case "sombre":
      return t("planche.warn.sombre");
    case "floue":
      return t("planche.warn.floue");
  }
}

/** Under half a pixel each way, no gesture can move anything. Read by the
 *  badge below and by the two crop gestures, so it is declared once. */
export const ROOM_EPSILON = 0.5;

/**
 * Whether the photograph prints dark — the photograph as it will print,
 * réglage included. A badge that judged the original would tell someone who
 * has just rescued a night shot that it is still too dark, which is the
 * badge lying about the book it claims to describe.
 *
 * Without an adjustment, and under one that adjusts nothing, this is the raw
 * mean to the bit: same cache, same value, same photos marked. The corrected
 * mean is computed at the read, from the histogram, and never cached beside
 * the raw one — so « Rendre à l'original » gives the original badge back at
 * once.
 *
 * The réglage handed in is the album's, never the draft: the caller passes
 * `reglagePose`, so a moving slider does not make the badge blink about a
 * réglage the album has not accepted yet.
 */
function estSombre(
  src: string,
  img: HTMLImageElement,
  reglage?: Reglage,
): boolean {
  if (!reglage || estIdentite(reglage)) {
    const brut = meanLuma(src, img);
    return brut !== undefined && brut < DARK_MEAN_LUMA;
  }
  const histo = histoLuma(src, img);
  return histo !== undefined && moyenneCorrigee(histo, reglage) < DARK_MEAN_LUMA;
}

/**
 * The three alerts and the one fact.
 *
 * Resolution and blur speak about the original, so they read the relevé:
 * the original's size, oriented, and the sharpness the analysis measured,
 * against the folder's own threshold. Darkness speaks about the print, so
 * it reads the thumbnail through the réglage, and needs its pixels. A
 * photo the relevé does not know carries no alert at all, and neither does
 * any photo while the relevé is on its way: a thumbnail of 1600 px proves
 * nothing about the original, and nothing is guessed. The preflight, which
 * reopens the originals, stays the authority at export time.
 */
export function badgesDe(
  src: string,
  img: HTMLImageElement | null,
  rect: Rect,
  mm: number,
  zoom: number,
  reglage: Reglage | undefined,
  releveAlbum: ReleveAlbum | null,
): Badges {
  const pixels = img !== null && img.naturalWidth > 0 ? img : null;
  let sansMarge = false;
  if (pixels) {
    const room = slidingRoom(
      { w: rect.w * mm, h: rect.h * mm },
      pixels.naturalWidth,
      pixels.naturalHeight,
      zoom,
    );
    sansMarge = room.x <= ROOM_EPSILON && room.y <= ROOM_EPSILON;
  }
  const m = releveAlbum?.photos[src];
  if (!releveAlbum || !m) return { alertes: [], sansMarge };
  const alertes: Alerte[] = [];
  const p = effectivePpi(rect, m.largeur, m.hauteur, zoom);
  if (p < MIN_EFFECTIVE_PPI) alertes.push({ code: "sous_resolution", ppi: Math.round(p) });
  // Darkness is a fact about the print, and reads through the adjustment.
  // The analysis's own exposure score does not: it is a scalar of the
  // original, and nothing can unfold it. See `analyze.rs::exposure_score`.
  if (pixels && estSombre(src, pixels, reglage)) alertes.push({ code: "sombre" });
  if (estFloue(m.nettete, releveAlbum)) alertes.push({ code: "floue" });
  return { alertes, sansMarge };
}

function estFloue(nettete: number, r: ReleveAlbum): boolean {
  return r.seuil_flou !== null && nettete <= r.seuil_flou;
}

/**
 * Ce que la table lumineuse dit d'une photo de la réserve, qui n'a pas de
 * case : trop petite pour une demi-page à 250 ppi dans le format de
 * l'album, ou floue. La demi-page coupe la page en deux sur sa longueur et
 * se tourne comme la photo ; le ppi s'y calcule comme dans une case, par
 * `effectivePpi`, sans nouveau seuil.
 */
export function alertesDeReserve(
  src: string,
  page: { w: number; h: number },
  releveAlbum: ReleveAlbum | null,
): Alerte[] {
  const m = releveAlbum?.photos[src];
  if (!releveAlbum || !m) return [];
  const demi = page.w >= page.h ? { w: page.w / 2, h: page.h } : { w: page.w, h: page.h / 2 };
  const [long, court] = [Math.max(demi.w, demi.h), Math.min(demi.w, demi.h)];
  const rect = m.largeur >= m.hauteur ? { w: long, h: court } : { w: court, h: long };
  const alertes: Alerte[] = [];
  const p = effectivePpi(rect, m.largeur, m.hauteur);
  if (p < MIN_EFFECTIVE_PPI) alertes.push({ code: "sous_resolution", ppi: Math.round(p) });
  if (estFloue(m.nettete, releveAlbum)) alertes.push({ code: "floue" });
  return alertes;
}
