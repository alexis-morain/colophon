// Le pack d'ornements, tenu une fois pour la vie de la fenêtre, et le seul
// endroit qui sait transformer un dessin en tracé.
//
// **Le pack vient du moteur, jamais d'une copie.** Le format existe pour que
// contribuer un ornement soit une entrée dans `pack.toml` et un `.svg`, sans
// une ligne de Rust ; une seconde liste ici en ferait aussi une ligne de
// TypeScript, et les deux dériveraient le jour où quelqu'un en oublie une.
//
// **Un seul tracé pour les deux rendus.** Le DOM pose un `<path d>`, le canvas
// un `Path2D` construit sur la même chaîne — donc deux images d'un ornement ne
// peuvent pas différer, parce qu'il n'y a qu'une chaîne. C'est le même
// principe que `scene.ts`, où l'alignement d'une ligne est calculé une fois
// dans l'assembleur plutôt qu'une fois par peintre.
//
// Le dessin arrive **déjà normalisé** : coordonnées absolues, droites et
// cubiques, `y` vers le bas comme un `viewBox`. `ornement.rs` a fait tout le
// travail de lecture, y compris replier `H`, `V` et le relatif ; il ne reste
// ici aucune interprétation de SVG.

import { langue, t } from "./i18n";

export type Famille = "fleuron" | "filet" | "separateur";

export type Segment =
  | { op: "vers"; x: number; y: number }
  | { op: "ligne"; x: number; y: number }
  | { op: "courbe"; x1: number; y1: number; x2: number; y2: number; x: number; y: number }
  | { op: "ferme" };

export type Chemin = {
  segments: Segment[];
  /** `fill-rule="evenodd"`, le seul attribut de style que le sous-ensemble
   *  garde : sans lui un fleuron ajouré se remplit plein. */
  evenodd: boolean;
};

/** `[min_x, min_y, largeur, hauteur]`, repère du SVG, `y` vers le bas. */
export type Dessin = { viewbox: [number, number, number, number]; chemins: Chemin[] };

export type Ornement = {
  id: string;
  famille: Famille;
  titre_fr: string;
  titre_en: string;
  licence: string;
  auteur: string;
  source: string;
  dessin: Dessin;
};

/** Le nom du seul pack livré, miroir d'`ornement.rs::PACK_INTERNE`.
 *
 *  Il ne voyage pas dans le dump : un ornement y porte son identifiant et pas
 *  le pack d'où il sort, parce que rien du dessin n'en dépend. C'est la pose
 *  qui l'écrit dans `album.json`, pour qu'un second pack puisse exister un
 *  jour sans que les identifiants aient à être uniques entre eux. */
export const PACK_INTERNE = "colophon";

let pack: Ornement[] = [];

/** Poser le pack. Appelé une fois par `bridge.ts`, au démarrage. */
export function setOrnements(o: Ornement[]): void {
  pack = o;
}

export function ornements(): Ornement[] {
  return pack;
}

/** L'ordre des familles à l'écran. Il est ici et pas dans le manifeste : une
 *  PR de données qui ajoute un fleuron en queue de `pack.toml` ne doit pas
 *  déplacer les groupes sous la main de quelqu'un. */
const FAMILLES: Famille[] = ["fleuron", "filet", "separateur"];

/** Le pack rangé par famille, dans cet ordre-là, les familles vides passées.
 *  Un groupe titré qui ne contient rien est un titre qui ment. */
export function parFamille(): [Famille, Ornement[]][] {
  const groupes: [Famille, Ornement[]][] = [];
  for (const f of FAMILLES) {
    const dedans = pack.filter((o) => o.famille === f);
    if (dedans.length > 0) groupes.push([f, dedans]);
  }
  return groupes;
}

/** Le nom d'une famille dans la langue de l'écran. Contrairement au titre d'un
 *  ornement, il ne voyage pas dans le pack : c'est un mot de l'interface, pas
 *  une donnée qu'une contribution apporte. */
export function titreDeFamille(f: Famille): string {
  return t(`ornement.famille.${f}`);
}

/** Un ornement par son identifiant, ou `undefined`.
 *
 *  **Un identifiant inconnu n'est pas une erreur** : un `album.json` se répare
 *  à la main, donc cet état est atteignable. Le moteur ne dessine rien et
 *  n'échoue pas ; l'écran fait pareil. */
export function ornementDe(id: string): Ornement | undefined {
  return pack.find((o) => o.id === id);
}

/** Le titre d'un ornement dans la langue de l'écran. Les deux voyagent dans le
 *  pack plutôt que dans `i18n.ts` : un ornement est de la donnée, et sa
 *  traduction arrive avec lui dans la PR qui l'ajoute. */
export function titre(o: Ornement): string {
  return langue() === "fr" ? o.titre_fr : o.titre_en;
}

/** L'attribut `d` d'un chemin normalisé.
 *
 *  Trois opérateurs et une fermeture, parce que c'est tout ce que le dessin
 *  contient après lecture. Les nombres sortent tels quels : ce sont les mêmes
 *  `f64` que l'émetteur PDF pose dans son flux. */
export function attributD(chemin: Chemin): string {
  const morceaux: string[] = [];
  for (const s of chemin.segments) {
    switch (s.op) {
      case "vers":
        morceaux.push(`M${s.x} ${s.y}`);
        break;
      case "ligne":
        morceaux.push(`L${s.x} ${s.y}`);
        break;
      case "courbe":
        morceaux.push(`C${s.x1} ${s.y1} ${s.x2} ${s.y2} ${s.x} ${s.y}`);
        break;
      case "ferme":
        morceaux.push("Z");
        break;
    }
  }
  return morceaux.join(" ");
}

export function attributViewBox(d: Dessin): string {
  return d.viewbox.join(" ");
}

/** Le rapport largeur/hauteur du dessin, que la boîte d'un ornement garde :
 *  le redimensionnement est proportionnel, jamais libre. Miroir de
 *  `ornement.rs::Dessin::rapport`. */
export function rapport(d: Dessin): number {
  return d.viewbox[2] / d.viewbox[3];
}

/** Peindre un ornement dans un rectangle du canvas, en millimètres et
 *  origine en haut à gauche — le repère de la scène.
 *
 *  Le canvas et le DOM consomment la **même** chaîne de tracé : `Path2D` lit
 *  un `d` de SVG, donc il n'y a pas ici une seconde façon de suivre une
 *  cubique. La rotation est posée par l'appelant, qui la pose déjà pour tout
 *  le reste de la planche. */
export function peindre(
  ctx: CanvasRenderingContext2D,
  dessin: Dessin,
  boite: { x: number; y: number; w: number; h: number },
  encre: string,
): void {
  const [minx, miny, vw, vh] = dessin.viewbox;
  ctx.save();
  ctx.translate(boite.x, boite.y);
  ctx.scale(boite.w / vw, boite.h / vh);
  ctx.translate(-minx, -miny);
  ctx.fillStyle = encre;
  for (const chemin of dessin.chemins) {
    ctx.fill(new Path2D(attributD(chemin)), chemin.evenodd ? "evenodd" : "nonzero");
  }
  ctx.restore();
}
