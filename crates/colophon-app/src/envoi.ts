// Ce que l'écran Envoi décide, hors du DOM : quel imprimeur il nomme, ce que
// fait son bouton, et la phrase qu'il dit une fois le dossier préparé.
//
// L'écran n'offre plus qu'un imprimeur. Cloudprinter est celui qu'on
// commande ; Prodigi et Lulu restent des profils du moteur et de la CLI
// (`--profil`), et ne s'affichent plus. Le PDF sans contrainte reste à un
// lien de distance : l'export gratuit, hors ligne et sans compte ne dépend
// d'aucun imprimeur.

import type { Preparation, Printer } from "./bridge";
import { t } from "./i18n";

/** L'imprimeur que l'écran nomme et prépare. */
export const PROFIL_ENVOI = "cloudprinter";
/** Le PDF sans contrainte, derrière le lien « Un autre imprimeur ? ». */
export const PROFIL_LIBRE = "generique";

/** Le profil que l'écran montre : le PDF sans contrainte quand on l'a
 *  choisi, Cloudprinter dans tous les autres cas. Null tant que le moteur
 *  n'a pas rendu la liste : l'écran attend plutôt que d'inventer. */
export function imprimeurAffiche(
  printers: Printer[] | null,
  profil: string,
): Printer | null {
  if (!printers) return null;
  const id = profil === PROFIL_LIBRE ? PROFIL_LIBRE : PROFIL_ENVOI;
  return printers.find((p) => p.id === id) ?? null;
}

/** Le bouton de l'écran. Chez qui relie deux fichiers, il prépare un
 *  dossier ; sinon il enregistre un PDF, comme il l'a toujours fait. */
export function actionEnvoi(p: Printer): { prepare: boolean; texte: string; titre: string } {
  return p.fichiers === "deux"
    ? {
        prepare: true,
        texte: t("envoi.preparer", { imprimeur: p.nom }),
        titre: t("envoi.preparer.titre"),
      }
    : { prepare: false, texte: t("envoi.exporter"), titre: t("envoi.exporter.titre") };
}

/** Une phrase : le dossier est prêt, ou ce que le contrôle relu sur ses
 *  fichiers y a trouvé. */
export function phrasePreparation(p: Preparation): string {
  const n = p.rapport.bloquants;
  if (p.rapport.ok) return t("envoi.prepare.ok", { dossier: p.dossier });
  return n === 1
    ? t("envoi.prepare.ko.un", { dossier: p.dossier })
    : t("envoi.prepare.ko", { dossier: p.dossier, n });
}
