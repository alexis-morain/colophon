// Les six codes de refus d'une police, nommés une fois, et le nom lisible
// d'une face. Le modèle est `reasons.ts` : codes côté moteur, libellés côté
// app, une seule table pour tous les écrans qui en parlent.
//
// Ici plutôt que dans le panneau, pour la même raison qu'il y a un
// `reasons.ts` : le panneau les affiche, la ligne de statut en cite une, et
// le jour où un troisième écran en parle il lit la même table.

import { Cle, t } from "./i18n";
import { PoliceOfferte } from "./bridge";

/** Les cinq refus du lecteur de polices, plus celui de la résolution :
 *  l'album nomme une face dont le fichier n'est plus dans son dossier. */
export const REFUS_KEYS = [
  "illisible",
  "embarquement_interdit",
  "bitmap_seulement",
  "cmap_illisible",
  "format_non_embarquable",
  "fichier_absent",
] as const;

/** Le libellé d'un refus, ou le code brut pour un code inconnu — comme
 *  `reasonLabel`, et pour la même raison : un code que l'app ne connaît pas
 *  se montre, il ne disparaît pas. */
export function refusLibelle(code: string): string {
  return (REFUS_KEYS as readonly string[]).includes(code)
    ? t(`police.refus.${code}` as Cle)
    : code;
}

/** Le nom lisible, élagué pour l'écran.
 *
 *  Le moteur colle famille (ID 1) et style (ID 2) — d'où « MuktaMahee Medium
 *  Regular » et « Helvetica Neue Regular » —, et il doit continuer à rendre
 *  ce que le fichier déclare : retirer le « Regular » final est une décision
 *  d'écran. Une face qui ne s'appelle *que* « Regular » garde son nom : ce
 *  qu'on élague est un style redondant, jamais le nom entier. */
export function nomLisible(p: { nom: string; famille?: string }): string {
  const court = p.nom.replace(/(.)\s+Regular$/, "$1");
  return court || p.nom || p.famille || "";
}

/** Une face que la plate-forme se réserve. macOS en porte des centaines —
 *  `.SF NS`, `.Apple SD Gothic NeoI`, `.ADT Slab Numeric` — et les nomme
 *  toutes par un point initial. Personne ne compose un album là-dedans, et
 *  vu à l'écran, un point trie avant une lettre : sans cette règle, les
 *  premières familles de la liste sont **toutes** internes et les vraies
 *  n'apparaissent qu'au filtre. */
function interne(famille: string): boolean {
  return famille.startsWith(".");
}

/** Les familles, dans l'ordre, chacune avec ses faces.
 *
 *  Rien n'est caché — le sélecteur montre tout, refus compris — mais les
 *  faces que la plate-forme se réserve passent après : l'ordre est une
 *  décision d'écran, comme « la face du projet en tête ».
 *
 *  Une face que le fichier ne nomme pas du tout — le seul refus qui coûte
 *  son nom — n'a pas de famille où se ranger et sort de la liste. */
export function parFamille(
  polices: PoliceOfferte[],
): { famille: string; faces: PoliceOfferte[] }[] {
  const map = new Map<string, PoliceOfferte[]>();
  for (const p of polices) {
    const cle = p.famille || p.nom || p.postscript;
    if (!cle) continue;
    const liste = map.get(cle);
    if (liste) liste.push(p);
    else map.set(cle, [p]);
  }
  return [...map.entries()]
    .map(([famille, faces]) => ({ famille, faces }))
    .sort(
      (a, b) =>
        Number(interne(a.famille)) - Number(interne(b.famille)) ||
        a.famille.localeCompare(b.famille),
    );
}

