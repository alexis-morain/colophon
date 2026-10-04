// Ce que l'écran Envoi décide, hors du DOM : quel imprimeur il nomme, ce que
// fait son bouton, et la phrase qu'il dit une fois le dossier préparé.
//
// L'écran n'offre plus qu'un imprimeur. Cloudprinter est celui qu'on
// commande ; Prodigi et Lulu restent des profils du moteur et de la CLI
// (`--profil`), et ne s'affichent plus. Le PDF sans contrainte reste à un
// lien de distance : l'export gratuit, hors ligne et sans compte ne dépend
// d'aucun imprimeur.

import type { Album } from "./album";
import type { Defaut, Preparation, Printer } from "./bridge";
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

/** Un geste offert sur une ligne de défaut d'Envoi. */
export type ActionDefaut = "voir" | "remplacer" | "retirer";

/** Les gestes d'un défaut `resolution`. Retirer marche toujours, même quand
 *  aucune case n'est assez petite pour cet original : c'est l'échappatoire
 *  qui ne manque jamais. Remplacer demande une réserve. Rien pour un défaut
 *  sans planche ni case, rien pour une autre règle. Et rien non plus quand la
 *  case ne porte plus la photo que le rapport nomme : le prévol lit le
 *  disque, et après une édition non enregistrée retirer viserait une autre
 *  photo. */
export function actionsDefaut(d: Defaut, album: Album, reserveVide: boolean): ActionDefaut[] {
  if (d.regle !== "resolution" || d.planche === undefined || d.case === undefined) return [];
  if (album.spreads[d.planche - 1]?.slots[d.case]?.src !== d.src) return [];
  return reserveVide ? ["voir", "retirer"] : ["voir", "remplacer", "retirer"];
}

/** La résolution qu'un défaut `resolution` mesure, lue dans sa cause : le
 *  moteur ne la porte pas en champ, et le prévol n'est pas à toucher pour
 *  ça. Null quand la cause ne la dit pas. */
export function ppiDe(d: Defaut): number | null {
  const m = / (\d+) ppi /.exec(d.cause);
  return m ? Number(m[1]) : null;
}
