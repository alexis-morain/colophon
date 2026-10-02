// Le menu contextuel, sans le DOM : ce qu'il contient pour chaque cible, où
// il s'ouvre, et comment le clavier le parcourt. `MenuContextuel.tsx` ne fait
// que le dessiner. Tout ce qui se décide ici se teste sans fenêtre, comme
// `popover.ts`.
//
// Trois cibles et pas une de plus (plan E, décision E2) : une photo, le
// papier nu d'une page, un objet libre. Le contenu de chaque menu est une
// liste fermée, dans l'ordre où elle s'affiche.

import { Cle } from "./i18n";

/** Ce qu'il y avait sous le pointeur au clic droit. */
export type Cible =
  | { type: "photo"; cell: number; src: string }
  | { type: "page"; droite: boolean; point: { x: number; y: number } }
  | { type: "objet"; index: number };

export type EntreeId =
  | "remplacer"
  | "retirer"
  | "copier"
  | "voir-original"
  | "informations"
  | "ajouter"
  | "bloc"
  | "ornement"
  | "couper"
  | "dupliquer"
  | "supprimer"
  | "reglages";

export type Entree = {
  id: EntreeId;
  cle: Cle;
  /** Inactive, et pourquoi : l'infobulle le dit. */
  desactive?: Cle;
};

/** Les entrées du menu d'une cible, dans l'ordre. `fiche` dit si la fiche
 *  photo (chantier F) est là : tant qu'elle ne l'est pas, ses deux entrées
 *  restent visibles et inactives, pour que le menu ne change pas de forme le
 *  jour où elle arrive. */
export function entreesPour(cible: Cible, options: { fiche: boolean }): Entree[] {
  const fiche = options.fiche ? undefined : ("ctx.fiche.plus.tard" as const);
  switch (cible.type) {
    case "photo":
      return [
        { id: "remplacer", cle: "ctx.remplacer" },
        { id: "retirer", cle: "ctx.retirer" },
        { id: "copier", cle: "ctx.copier" },
        { id: "voir-original", cle: "ctx.voir.original", desactive: fiche },
        { id: "informations", cle: "ctx.informations", desactive: fiche },
      ];
    case "page":
      return [
        { id: "ajouter", cle: "ctx.ajouter.photo" },
        { id: "bloc", cle: "objet.ajouter" },
        { id: "ornement", cle: "ornement.ajouter" },
      ];
    case "objet":
      return [
        { id: "copier", cle: "ctx.copier" },
        { id: "couper", cle: "ctx.couper" },
        { id: "dupliquer", cle: "ctx.dupliquer" },
        { id: "supprimer", cle: "ctx.supprimer" },
        { id: "reglages", cle: "ctx.reglages" },
      ];
  }
}

/** L'écart entre le menu et le bord de la fenêtre. */
const ECART = 8;

/** Au pointeur, rabattu dans la fenêtre : un menu ouvert près du bord droit
 *  ou du bas se décale pour tenir entier, il ne se coupe jamais. */
export function placerMenu(
  point: { x: number; y: number },
  taille: { w: number; h: number },
  fenetre: { w: number; h: number },
): { left: number; top: number } {
  return {
    left: Math.max(ECART, Math.min(point.x, fenetre.w - taille.w - ECART)),
    top: Math.max(ECART, Math.min(point.y, fenetre.h - taille.h - ECART)),
  };
}

/**
 * Le rang qu'une touche désigne dans une liste d'entrées, ou `null` quand la
 * touche ne navigue pas. Flèches en boucle, Début et Fin, et la première
 * lettre d'un libellé, cherchée après le rang courant puis depuis le début.
 * Une entrée inactive se saute, comme dans un menu du système.
 */
export function rangSuivant(
  rang: number | null,
  touche: string,
  libelles: string[],
  actives: boolean[],
): number | null {
  const n = libelles.length;
  if (n === 0) return null;
  const ok = (i: number) => actives[i] !== false;
  const pas = (depuis: number | null, sens: 1 | -1): number | null => {
    let i = depuis === null ? (sens === 1 ? -1 : n) : depuis;
    for (let k = 0; k < n; k += 1) {
      i = (i + sens + n) % n;
      if (ok(i)) return i;
    }
    return null;
  };
  switch (touche) {
    case "ArrowDown":
      return pas(rang, 1);
    case "ArrowUp":
      return pas(rang, -1);
    case "Home":
      return pas(null, 1);
    case "End":
      return pas(null, -1);
  }
  if (touche.length !== 1 || /\s/.test(touche)) return null;
  const lettre = touche.toLocaleLowerCase();
  const commence = (i: number) =>
    ok(i) && libelles[i].trim().toLocaleLowerCase().startsWith(lettre);
  const depart = rang === null ? 0 : rang + 1;
  for (let k = 0; k < n; k += 1) {
    const i = (depart + k) % n;
    if (commence(i)) return i;
  }
  return null;
}
