// La veille : est-ce que l'application demande à GitHub, au lancement, s'il
// existe une version plus récente.
//
// **Pourquoi un interrupteur, et pourquoi il est allumé.** Colophon se
// distribue en dehors de toute boutique et sans signature d'éditeur : la
// seule chose qui amène un correctif de sûreté sur la machine de quelqu'un
// est cette question posée au démarrage, et la couper par défaut coûterait
// plus cher que ce qu'elle rapporte. Mais elle sort de la machine, donc elle
// se dit, elle se nomme, et elle se coupe. Ce qui part est une requête HTTPS
// vers le flux des versions ; ce qui arrive chez GitHub est l'adresse IP et
// l'agent utilisateur de cette requête, rien du dossier, rien de l'album,
// aucun identifiant que Colophon fabriquerait. Rien n'est jamais installé
// sans un clic : le bandeau attend.
//
// `veille` n'est pas `maj`. La veille dit si on regarde ; `maj`, dans
// `App`, est la version trouvée quand on a regardé.
//
// Même forme que `rendu.ts` et `i18n.ts`, pour la même raison : un magasin,
// un crochet, une clé de `localStorage`, aucune bibliothèque.

import { useSyncExternalStore } from "react";

export type Veille = "au-lancement" | "jamais";

const CLE = "colophon.maj";

let courante: Veille = (() => {
  try {
    const garde =
      typeof localStorage === "undefined" ? null : localStorage.getItem(CLE);
    if (garde === "au-lancement" || garde === "jamais") return garde;
  } catch {
    /* un stockage bloqué ne coûte que la mémoire du choix */
  }
  return "au-lancement";
})();

const abonnes = new Set<() => void>();

export function veille(): Veille {
  return courante;
}

export function setVeille(v: Veille): void {
  if (v === courante) return;
  courante = v;
  try {
    if (typeof localStorage !== "undefined") localStorage.setItem(CLE, v);
  } catch {
    /* idem */
  }
  abonnes.forEach((f) => f());
}

/** S'abonne au changement de veille, et rend de quoi se désabonner. */
export function abonneVeille(f: () => void): () => void {
  abonnes.add(f);
  return () => {
    abonnes.delete(f);
  };
}

/** Rend le composant appelant sensible au changement de veille. */
export function useVeille(): Veille {
  return useSyncExternalStore(
    abonneVeille,
    () => courante,
    () => courante,
  );
}
