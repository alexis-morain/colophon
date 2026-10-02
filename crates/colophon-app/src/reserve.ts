// Le choix d'une photo de la réserve, sans le DOM : ce qu'il fait à l'album
// quand on choisit une candidate, et ce qu'il répond quand il ne peut pas.
// `ChoixReserve.tsx` dessine la liste ; ce qui change l'album est ici, et
// se teste sur une planche de fixture.

import { Album, Slot } from "./album";
import { Candidat } from "./bridge";
import { addBlocker, addPhoto, insertSpread, placePhoto } from "./edits";

/** D'où le choix vient : « Ajouter une photo… » sur une page, ou
 *  « Remplacer par… » sur une case. */
export type ModeChoix = { type: "ajouter" } | { type: "remplacer"; cell: number };

/** La case qu'une candidate devient : sa source et le focal que le moteur a
 *  gardé pour elle (un visage, sinon le défaut du Composer). */
export function caseDe(c: Candidat): Slot {
  return { src: c.src, focal: c.focal };
}

export type Resultat =
  | { type: "posee"; album: Album; at: number }
  | { type: "remplacee"; album: Album }
  /** La planche n'accueille pas une photo de plus, ou c'est une page de
   *  texte : l'album ne bouge pas, le popover offre la planche d'après. */
  | { type: "pleine"; raison: "target_full" | "target_text" }
  | { type: "refus" };

/**
 * Poser une candidate selon le mode. Ajouter fait grandir la planche
 * (`addPhoto`) ; remplacer est `placePhoto` tel quel, et la photo d'avant
 * repart dans la réserve d'elle-même, puisque la réserve est ce que l'album
 * ne montre pas.
 */
export function poser(album: Album, at: number, mode: ModeChoix, c: Candidat): Resultat {
  const photo = caseDe(c);
  if (mode.type === "remplacer") {
    const suivant = placePhoto(album, at, mode.cell, photo);
    return suivant === album ? { type: "refus" } : { type: "remplacee", album: suivant };
  }
  const blocage = addBlocker(album, at, photo.src);
  if (blocage === "target_full" || blocage === "target_text") {
    return { type: "pleine", raison: blocage };
  }
  const suivant = addPhoto(album, at, photo);
  return suivant === album ? { type: "refus" } : { type: "posee", album: suivant, at };
}

/** Pourquoi « Ajouter » ne pourra pas, avant même de choisir : le popover
 *  le dit d'abord et offre la planche d'après, plutôt que de faire choisir
 *  une photo pour la refuser ensuite. */
export function pleineAvant(album: Album, at: number): "target_full" | "target_text" | null {
  const b = addBlocker(album, at, "");
  return b === "target_full" || b === "target_text" ? b : null;
}

/** Une planche vide insérée après `at`, qui reçoit la candidate : c'est
 *  l'issue d'une planche pleine. Rend l'album et le rang de la planche
 *  neuve, où le livre doit aller. */
export function poserApres(album: Album, at: number, c: Candidat): { album: Album; at: number } {
  const inseree = insertSpread(album, at, "vide");
  return { album: addPhoto(inseree, at + 1, caseDe(c)), at: at + 1 };
}
