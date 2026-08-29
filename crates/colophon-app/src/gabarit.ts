// Ce qu'un gabarit s'appelle à l'écran, et ce qui fait que deux gabarits
// sont le même choix.
//
// Le catalogue offert compte 209 familles, et le moteur en juge jusqu'à 171
// compatibles pour une planche de quatre photos. Ce n'est pas une liste,
// c'est un mur — et la moitié de ce mur est fait de doublons visuels : la
// même disposition avec une bande de légende de huit millimètres
// (`…_b8`, invisible à la taille d'une vignette) et la même disposition
// avec une autre forme de cellule (`…1x2f` contre `…1x2l`, une lettre dans
// un nom que personne ne lit).
//
// Le sélecteur ne montre donc pas des gabarits, il montre des
// **dispositions** : combien de cases sur chaque page, en combien de
// rangées. Le gabarit posé, lui, est la variante de cette disposition que
// **ces photos-là** cadrent le mieux — et ce « mieux » vient du moteur
// (`gabarit::trahison`, remonté par `gabarits_compatibles`), jamais d'une
// arithmétique refaite ici : l'aptitude d'un gabarit a une seule
// définition dans ce projet.
//
// Le nom généré est une grammaire, celle que `gabarit::parse_genere` relit
// pour réparer un `album.json` à la main : `g_<page>_<page>[_b<mm>]`, où une
// page est `v` (rien), `p` (pleine page) ou `<colonnes>x<rangées><forme>`.
// Elle est relue ici pour une seule chose, le libellé, et `gabarit.test.ts`
// exige qu'aucun nom du dump n'y échappe : le jour où la grammaire bouge,
// c'est ce test qui le dit, pas un `g_1x2f_1x2f` affiché à quelqu'un.

import { templateCapacity, templates } from "./album";
import { Cle, FR, t } from "./i18n";

/** Une page d'une disposition. */
export type PageForme =
  | { sorte: "vide" }
  | { sorte: "pleine" }
  | { sorte: "grille"; cols: number; rangs: number };

/** Une disposition : les deux pages, **sans côté**. Le sélecteur retourne
 *  la planche selon sa parité comme le fait le Composer, donc « à gauche »
 *  et « à droite » ne veulent rien dire ici ; la paire est normalisée pour
 *  que le recto et son verso soient une seule entrée. */
export type Forme = { a: PageForme; b: PageForme };

/** La famille derrière un nom de gabarit, le verso replié. */
export function familyOf(template: string): string {
  return template.endsWith("_verso")
    ? template.slice(0, -"_verso".length)
    : template;
}

/** La face qu'une disposition prend sur cette planche : le verso sur les
 *  planches impaires quand la variante existe, comme le fait le Composer
 *  (`layout.rs::with_flip`). Un retournement d'affichage, pas un gabarit de
 *  plus : la table du moteur n'est pas touchée. */
export function faceFor(family: string, index: number): string {
  const verso = `${family}_verso`;
  if (index % 2 === 1 && templates().some(([t]) => t === verso)) return verso;
  return family;
}

/** Les dispositions des familles historiques, celles que le Composer pose.
 *
 *  Elles vivent dans `gabarit::familles()`, côté moteur, et la seule chose
 *  qui en descend jusqu'ici est le rectangle de chaque case — de quoi
 *  dessiner la vignette, pas de quoi dire « deux par page ». Ces dix-huit
 *  lignes sont donc le seul endroit de l'application qui redit une forme du
 *  moteur, et `gabarit.test.ts` les confronte au dump case par case :
 *  capacité déclarée contre cases comptées. Une famille ajoutée là-bas sans
 *  ligne ici sort en clair, ce qui se voit, plutôt qu'en faux, ce qui ne se
 *  voit pas. */
const HISTORIQUES: Record<string, [string, string]> = {
  full1: ["v", "p"],
  solo: ["v", "1x1"],
  solo_paysage: ["v", "1x1"],
  solo_pano: ["v", "1x1"],
  solo_etroit: ["v", "1x1"],
  solo_carre: ["v", "1x1"],
  duo: ["1x1", "1x1"],
  duo_portrait: ["1x1", "1x1"],
  duo_paysage: ["1x1", "1x1"],
  duo_etroit: ["1x1", "1x1"],
  duo_pano: ["1x1", "1x1"],
  trio: ["1x2", "p"],
  trio_portrait: ["2x1", "p"],
  quad: ["1x2", "1x2"],
  quad_portrait: ["2x1", "2x1"],
  quad_etroit: ["2x1", "2x1"],
  quad_pano: ["1x2", "1x2"],
  six: ["2x2", "1x2"],
  octo: ["2x2", "2x2"],
};

function parsePage(code: string): PageForme | null {
  if (code === "v") return { sorte: "vide" };
  if (code === "p") return { sorte: "pleine" };
  const m = /^(\d+)x(\d+)[a-z]?$/.exec(code);
  if (!m) return null;
  return { sorte: "grille", cols: Number(m[1]), rangs: Number(m[2]) };
}

/** L'ordre canonique d'une paire : rien, puis la pleine page, puis les
 *  grilles par nombre de cases. Deux pages échangées sont la même
 *  disposition, et c'est ce qui replie un verso sur son recto. */
function rang(p: PageForme): number {
  if (p.sorte === "vide") return 0;
  if (p.sorte === "pleine") return 1;
  return 2 + p.cols * p.rangs * 100 + p.cols;
}

function normalise(a: PageForme, b: PageForme): Forme {
  return rang(a) <= rang(b) ? { a, b } : { a: b, b: a };
}

/** La disposition d'un gabarit, ou `null` pour un nom sans cases (une
 *  planche de texte, la page de garde, le colophon). */
export function formeDe(template: string): Forme | null {
  const famille = familyOf(template);
  const hist = HISTORIQUES[famille];
  if (hist) {
    const a = parsePage(hist[0]);
    const b = parsePage(hist[1]);
    return a && b ? normalise(a, b) : null;
  }
  const parts = famille.split("_");
  if (parts.length < 3 || parts.length > 4 || parts[0] !== "g") return null;
  if (parts.length === 4 && !/^b\d+$/.test(parts[3])) return null;
  const a = parsePage(parts[1]);
  const b = parsePage(parts[2]);
  return a && b ? normalise(a, b) : null;
}

/** Le nom d'une disposition, celui sous lequel le sélecteur la regroupe. */
export function cleDeForme(f: Forme): string {
  const page = (p: PageForme) =>
    p.sorte === "grille" ? `${p.cols}x${p.rangs}` : p.sorte === "vide" ? "v" : "p";
  return `${page(f.a)}|${page(f.b)}`;
}

/** Combien de photos une page porte. Exporté pour `gabarit.test.ts`, qui
 *  confronte ce compte aux rectangles du dump : c'est lui qui attrape une
 *  ligne fausse dans la table des familles historiques. */
export function cases(p: PageForme): number {
  if (p.sorte === "vide") return 0;
  if (p.sorte === "pleine") return 1;
  return p.cols * p.rangs;
}

/** Un nombre en toutes lettres, jusqu'à ce qu'une page peut porter ; au-delà
 *  le chiffre, qui est laid mais vrai. */
function nombre(n: number): string {
  const cle = `gabarit.n.${n}` as Cle;
  return cle in FR ? t(cle) : String(n);
}

/** Ce que fait une page, en minuscules : le libellé s'assemble par morceaux
 *  et ne prend sa majuscule qu'à la fin. */
function disposition(p: PageForme): string {
  if (p.sorte !== "grille") return "";
  const n = nombre(p.cols * p.rangs);
  if (p.cols === 1 && p.rangs === 1) return n;
  if (p.rangs === 1) return t("gabarit.disp.ligne", { n });
  if (p.cols === 1) return t("gabarit.disp.colonne", { n });
  if (p.cols === p.rangs) return t("gabarit.disp.grille", { n });
  return t("gabarit.disp.grille.dim", { n, c: p.cols, r: p.rangs });
}

const majuscule = (s: string) => s.charAt(0).toLocaleUpperCase() + s.slice(1);

/** Le libellé d'une disposition. Le nombre de photos n'y est pas : le
 *  sélecteur groupe par nombre, et le répéter sur chaque case ferait lire
 *  « 4 photos » cinq fois pour ne distinguer aucune des cinq. */
export function libelleForme(f: Forme): string {
  const { a, b } = f;
  if (a.sorte === "vide") {
    if (b.sorte === "pleine") return majuscule(t("gabarit.forme.pleine_page"));
    if (b.sorte === "grille" && b.cols === 1 && b.rangs === 1) {
      return majuscule(t("gabarit.forme.cadree"));
    }
    return majuscule(t("gabarit.forme.une_page", { d: disposition(b) }));
  }
  if (a.sorte === "pleine" && b.sorte === "pleine") {
    return majuscule(t("gabarit.forme.deux_pleines"));
  }
  if (a.sorte === "pleine") {
    // « une pleine page, une » se lit comme une phrase coupée : la case
    // seule reprend le mot qui la nomme partout ailleurs.
    if (b.sorte === "grille" && b.cols === 1 && b.rangs === 1) {
      return majuscule(t("gabarit.forme.pleine_et_une"));
    }
    return majuscule(t("gabarit.forme.pleine_et", { d: disposition(b) }));
  }
  if (a.sorte === "grille" && b.sorte === "grille" && cases(a) === cases(b) && a.cols === b.cols) {
    return majuscule(t("gabarit.forme.par_page", { d: disposition(a) }));
  }
  return majuscule(
    t("gabarit.forme.deux_cotes", { a: disposition(a), b: disposition(b) }),
  );
}

/** Le libellé d'un gabarit : celui de sa disposition, ou l'entrée du
 *  dictionnaire pour les planches sans photo. Un nom qu'aucune des deux
 *  routes ne reconnaît s'affiche tel quel, ce qui reste le défaut honnête. */
export function libelleGabarit(template: string): string {
  const famille = familyOf(template);
  const cle = `gabarit.${famille}` as Cle;
  if (cle in FR) return t(cle);
  const f = formeDe(template);
  return f ? libelleForme(f) : famille;
}

/** Une entrée du sélecteur : une disposition, le gabarit qu'elle poserait
 *  sur cette planche, et combien de photos elle tient. */
export type Choix = { cle: string; template: string; capacite: number; libelle: string };

/**
 * Les dispositions offertes, une par entrée, la meilleure variante dans
 * chacune.
 *
 * `notes` est ce que le moteur a jugé compatible, chaque nom avec sa
 * trahison ; `courant` est le gabarit de la planche, qui entre toujours,
 * parce que l'état d'une planche n'est jamais un cul-de-sac.
 *
 * Le classement dans une disposition : la bande de légende en dernier —
 * le Composer n'en pose aucune, et la garder par défaut changerait la
 * mise en page de toutes les légendes d'un album pour un choix qui ne
 * parle que de cadres — puis la plus petite trahison, puis l'ordre du
 * catalogue. Le tri des entrées entre elles : par capacité décroissante,
 * la disposition courante en tête de son groupe.
 */
export function choixOfferts(
  notes: [string, number][],
  courant: string,
): Choix[] {
  const paquets = new Map<string, { template: string; note: number; bande: boolean }>();
  const retenir = (nom: string, note: number) => {
    const f = formeDe(nom);
    if (!f) return;
    const cle = cleDeForme(f);
    const bande = /_b\d+$/.test(familyOf(nom));
    const vu = paquets.get(cle);
    const mieux =
      !vu ||
      (vu.bande && !bande) ||
      (vu.bande === bande && note < vu.note);
    if (mieux) paquets.set(cle, { template: familyOf(nom), note, bande });
  };
  for (const [nom, note] of notes) retenir(nom, note);
  // La disposition de la planche entre par la porte de service : elle n'est
  // pas forcément compatible avec ses propres photos (un recadrage l'a
  // peut-être rendue inapte), et elle doit rester visible et cochée.
  const fCourant = formeDe(courant);
  if (fCourant && !paquets.has(cleDeForme(fCourant))) {
    paquets.set(cleDeForme(fCourant), {
      template: familyOf(courant),
      note: 1,
      bande: false,
    });
  }

  const cleCourante = fCourant ? cleDeForme(fCourant) : "";
  return [...paquets.entries()]
    .map(([cle, { template }]) => ({
      cle,
      template,
      capacite: templateCapacity(template),
      libelle: libelleGabarit(template),
    }))
    .sort(
      (x, y) =>
        y.capacite - x.capacite ||
        Number(y.cle === cleCourante) - Number(x.cle === cleCourante) ||
        x.libelle.localeCompare(y.libelle),
    );
}
