// Le dossier de photos de l'album ouvert, tel que l'écran le dit.
//
// `introuvable` : le dossier a disparu ou changé de place ; l'aperçu tourne
// sur les vignettes, l'export pleine résolution ne marchera pas.
// `a-revoir` : la version Mac App Store ne peut plus le lire (pas de
// marque-page, ou un marque-page que le système ne résout plus). Celui-là a
// une issue, un bouton qui rouvre le choix du dossier, et il passe devant :
// sous le bac à sable, « introuvable » peut n'être qu'un accès fermé.

import type { OpenedAlbum } from "./album";
import { t } from "./i18n";

export type EtatRacine = "present" | "introuvable" | "a-revoir";

export function etatRacine(
  o: Pick<OpenedAlbum, "root_present" | "dossier_a_revoir">,
): EtatRacine {
  if (o.dossier_a_revoir) return "a-revoir";
  return o.root_present ? "present" : "introuvable";
}

/** Pourquoi le dossier rechoisi n'est pas celui de l'album, en phrase :
 *  le moteur refuse par un code (`signet::dossier_revu`), traduit ici. Null
 *  pour toute autre erreur, qui garde la phrase générique. */
export function refusRevoir(erreur: string): string | null {
  const nom = /refus-revoir:autre-nom:(.*)$/.exec(erreur);
  if (nom) return t("racine.revoir.autre.nom", { nom: nom[1] });
  const m = /refus-revoir:manquantes:(\d+):(\d+)/.exec(erreur);
  if (m) return t("racine.revoir.manquantes", { n: m[1], total: m[2] });
  return null;
}
