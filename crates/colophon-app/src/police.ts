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


/** Une des dix voix du choix court, et les familles qui la portent selon
 *  la machine. La première présente gagne : la liste va du plus courant au
 *  plus rare, macOS puis Windows puis les libres. */
type Voix = { cle: string; familles: string[] };

/**
 * Les dix voix, et rien de plus.
 *
 * Le panneau montrait les 787 faces de la machine, familles internes en
 * tête, dans un mur qu'aucun filtre ne rendait choisissable — et personne
 * ne compose un album en parcourant huit cents noms. Ce qui se choisit
 * ici, c'est une **voix** : une linéale neutre, une humaniste, une
 * géométrique, un romain classique, un romain de texte, une didone, un
 * égyptien, une machine à écrire, un romain élégant, une chasse fixe. Dix
 * cases, une famille par case, la première que cette machine porte.
 *
 * C'est un raccourci, jamais une clôture : « toutes les polices » ouvre la
 * liste entière, filtre compris, refus compris. Un raccourci qui cacherait
 * définitivement une police installée serait le défaut que ce panneau
 * existe pour éviter.
 *
 * Aucun de ces noms n'atteint jamais une feuille de style : ils servent à
 * *reconnaître* une famille dans ce que le moteur a listé, et ce qui se
 * dessine ensuite, ce sont ses octets (`specimen.ts`).
 */
const VOIX: Voix[] = [
  {
    cle: "grotesque",
    familles: ["Helvetica Neue", "Helvetica", "Arial", "Inter", "Segoe UI", "Roboto", "Liberation Sans"],
  },
  {
    cle: "humaniste",
    familles: ["Optima", "Gill Sans", "Lucida Grande", "Candara", "Trebuchet MS", "Verdana", "Tahoma"],
  },
  {
    cle: "geometrique",
    familles: ["Avenir Next", "Avenir", "Futura", "Century Gothic", "Poppins", "Montserrat"],
  },
  {
    cle: "ancien",
    familles: ["Palatino", "Palatino Linotype", "Book Antiqua", "Iowan Old Style", "Garamond", "EB Garamond", "Hoefler Text"],
  },
  {
    cle: "texte",
    familles: ["Georgia", "Charter", "Cambria", "Times New Roman", "Times", "Constantia", "Liberation Serif"],
  },
  {
    cle: "didone",
    familles: ["Didot", "Bodoni 72", "Bodoni MT", "Playfair Display"],
  },
  {
    cle: "egyptienne",
    familles: ["Superclarendon", "Rockwell", "Roboto Slab", "Zilla Slab"],
  },
  {
    cle: "machine",
    familles: ["American Typewriter", "Courier New", "Courier", "Nimbus Mono PS"],
  },
  {
    cle: "elegant",
    familles: ["Baskerville", "Big Caslon", "Cochin", "Perpetua", "Libre Baskerville"],
  },
  {
    cle: "mono",
    familles: ["Menlo", "Consolas", "Monaco", "SF Mono", "Andale Mono", "DejaVu Sans Mono"],
  },
];

/** La face droite d'une famille : celle que le fichier appelle du nom de
 *  sa famille, ou celle-ci suivie de « Regular ». À défaut, le nom le plus
 *  court qui ne s'annonce ni italique ni oblique — un album se compose dans
 *  la droite, et personne ne va chercher l'italique d'Optima au moment de
 *  choisir une voix. */
function droite(famille: string, faces: PoliceOfferte[]): PoliceOfferte | null {
  const bonnes = faces.filter((f) => !f.refus);
  if (bonnes.length === 0) return null;
  const exact = bonnes.find(
    (f) => f.nom === famille || f.nom === `${famille} Regular`,
  );
  if (exact) return exact;
  const droites = bonnes.filter((f) => !/(italic|oblique|italique)/i.test(f.nom));
  return (droites.length > 0 ? droites : bonnes).reduce((a, b) =>
    b.nom.length < a.nom.length ? b : a,
  );
}

/** Les dix familles suggérées, dans l'ordre des voix. Moins de dix quand la
 *  machine ne porte rien d'une voix : compléter avec une deuxième famille
 *  d'une voix déjà servie rendrait le nombre et perdrait ce que la liste
 *  promet, qui est d'être variée. */
export function selection(polices: PoliceOfferte[]): PoliceOfferte[] {
  const parNom = new Map<string, PoliceOfferte[]>();
  for (const p of polices) {
    const cle = (p.famille || p.nom).toLocaleLowerCase();
    const liste = parNom.get(cle);
    if (liste) liste.push(p);
    else parNom.set(cle, [p]);
  }
  const prises = new Set<string>();
  const out: PoliceOfferte[] = [];
  for (const voix of VOIX) {
    for (const famille of voix.familles) {
      const cle = famille.toLocaleLowerCase();
      if (prises.has(cle)) continue;
      const face = droite(famille, parNom.get(cle) ?? []);
      if (!face) continue;
      prises.add(cle);
      out.push(face);
      break;
    }
  }
  return out;
}

/** La voix d'une face suggérée, pour la note sous son nom. `selection`
 *  rend les faces dans l'ordre des voix retenues, donc l'index suffirait —
 *  mais un index qui veut dire une clé est exactement le genre de lien
 *  qu'un tri futur casserait sans bruit. */
export function voixDe(p: PoliceOfferte): string | null {
  const cle = (p.famille || p.nom).toLocaleLowerCase();
  const voix = VOIX.find((v) =>
    v.familles.some((f) => f.toLocaleLowerCase() === cle),
  );
  return voix ? voix.cle : null;
}
