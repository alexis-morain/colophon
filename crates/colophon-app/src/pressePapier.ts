// Le presse-papier d'un objet libre : ce qu'on pose quand on copie, ce qu'on
// accepte quand on colle.
//
// C'est celui du système, par les événements `copy`, `cut` et `paste` : le
// menu Édition les déclenche comme le clavier, et un champ de saisie garde
// les siens. On y pose deux choses. L'objet tel qu'`album.json` le stocke,
// sous un type à nous, plus la planche d'où il vient (`de`) pour que le
// collage sache s'il doit décaler. Et le texte du bloc en texte brut, pour
// qu'un autre programme reçoive au moins la phrase.
//
// **`lire` est la seule porte d'entrée**, et ce qui arrive par là vient de
// n'importe où. Elle reconstruit un objet champ par champ au lieu de faire
// confiance au JSON : un champ glissé à côté ne passe pas, une boîte qui ne
// se dessine pas non plus. Un champ additif du modèle (A-s1 en prépare deux)
// devra s'ajouter ici, sinon le collage le perdra.

import { Alignement, Objet } from "./album";
import { PACK_INTERNE } from "./ornement";

/** Le type sous lequel un objet voyage. */
export const TYPE_OBJET = "application/x-colophon-objet";

/** Un objet reçu du presse-papier : l'objet, et la planche d'où il vient.
 *  `de` sert au collage et ne va jamais dans l'album. */
export type ObjetColle = Objet & { de?: number };

/** Sous 4 mm une boîte ne se prend plus à la souris : c'est la butée du
 *  redimensionnement. Elle vaut pour le grand côté seulement, parce qu'un
 *  filet de 20:1 naît à 3,26 mm de haut et doit rester copiable. */
const COTE_MIN = 4;

const ALIGNEMENTS: Alignement[] = ["gauche", "centre", "droite"];

/** Les deux formes d'un objet copié, par type de presse-papier. */
export function serialiser(objet: Objet, de: number): Record<string, string> {
  return {
    [TYPE_OBJET]: JSON.stringify({ ...objet, de }),
    "text/plain": objet.type === "texte" ? objet.texte : "",
  };
}

const fini = (v: unknown): v is number =>
  typeof v === "number" && Number.isFinite(v);

/** L'objet d'un collage, ou `null` si ce n'en est pas un. Ne lève jamais. */
export function lire(data: Pick<DataTransfer, "getData">): ObjetColle | null {
  const brut = data.getData(TYPE_OBJET);
  if (!brut) return null;
  let o: Record<string, unknown>;
  try {
    const v: unknown = JSON.parse(brut);
    if (typeof v !== "object" || v === null || Array.isArray(v)) return null;
    o = v as Record<string, unknown>;
  } catch {
    return null;
  }
  const { x, y, w, h, angle, de } = o;
  if (!fini(x) || !fini(y) || !fini(w) || !fini(h)) return null;
  if (w <= 0 || h <= 0 || Math.max(w, h) < COTE_MIN) return null;
  if (angle !== undefined && !fini(angle)) return null;
  const boite = {
    x,
    y,
    w,
    h,
    ...(angle ? { angle } : {}),
    ...(Number.isInteger(de) ? { de: de as number } : {}),
  };
  if (o.type === "texte") {
    const { texte, taille_pt, interligne_mm, alignement } = o;
    if (typeof texte !== "string" || !fini(taille_pt) || taille_pt <= 0) return null;
    if (interligne_mm !== undefined && !fini(interligne_mm)) return null;
    if (alignement !== undefined && !ALIGNEMENTS.includes(alignement as Alignement)) {
      return null;
    }
    return {
      ...boite,
      type: "texte",
      texte,
      taille_pt,
      ...(interligne_mm !== undefined ? { interligne_mm } : {}),
      ...(alignement !== undefined ? { alignement: alignement as Alignement } : {}),
    };
  }
  if (o.type === "ornement") {
    const { pack, id } = o;
    if (pack !== PACK_INTERNE || typeof id !== "string" || id === "") return null;
    return { ...boite, type: "ornement", pack, id };
  }
  return null;
}
