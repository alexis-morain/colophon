// Voir une face avant de la choisir.
//
// Le sélecteur de polices écrivait ses huit cents noms dans la police de
// l'interface : « Didot » et « American Typewriter » y avaient exactement
// la même voix, celle d'aucune des deux, et choisir se faisait à l'aveugle
// — on posait une face dans l'album pour découvrir de quoi elle avait
// l'air, puis on recommençait.
//
// Un spécimen est donc **les octets que l'émetteur embarquerait**, obtenus
// par la même extraction que le choix (`police_apercu`, qui n'écrit rien à
// côté de l'album : c'est ce qui sépare regarder de choisir), posés sous
// une famille interne à ce rang-là. La règle de `font.ts` tient ici mot
// pour mot : **jamais `font-family: "Helvetica Neue"`**. Nommer une face
// installée marcherait sur cette machine, dessinerait le crénage du
// navigateur que le PDF ne dessine pas, et ne montrerait rien du tout sur
// une machine qui ne l'a pas — un défaut invisible là où on le fabrique.
//
// Deux plafonds, et ils sont la raison pour laquelle ce module tient un
// compte. Une face CJK sortie de sa collection pèse des dizaines de
// mégaoctets (28,6 Mo mesurés pour Heiti TC Light) ; une liste qui en
// charge quarante mangerait le gigaoctet pour afficher des noms. Au-delà,
// le nom reste dans la police de l'interface : c'est une moins bonne
// réponse que le spécimen, et une bien meilleure que la mémoire de la
// machine.

import { policeApercu } from "./bridge";

/** Ce qu'une face a le droit de peser pour un spécimen. Au-delà, le moteur
 *  refuse plutôt que de la faire traverser le pont. */
const PLAFOND = 4 * 1024 * 1024;

/** Ce que tous les spécimens ensemble ont le droit de peser. */
const BUDGET = 24 * 1024 * 1024;

/** La famille sous laquelle un rang est posé. Interne, comme celle de
 *  `font.ts` : rien sur aucune machine ne s'appelle comme ça, donc la pile
 *  ne peut résoudre que vers les octets qu'on a posés. */
const famille = (rang: number) => `colophon-apercu-${rang}`;

type Etat = { famille: string } | { famille: null };

const connus = new Map<number, Etat>();
const enCours = new Map<number, Promise<string | null>>();
const posees = new Map<number, FontFace>();
let depense = 0;

/** La famille d'un rang déjà posé, sans rien demander. `null` tant qu'on
 *  ne sait pas, et pour une face qu'on ne montrera pas. */
export function familleDeja(rang: number): string | null {
  return connus.get(rang)?.famille ?? null;
}

/**
 * Poser la face de ce rang et rendre sa famille, ou `null`.
 *
 * `null` n'est pas une panne : c'est une face trop lourde, une face que le
 * moteur refuse d'extraire, ou un budget épuisé. L'appelant écrit alors le
 * nom dans la police de l'interface, ce qu'il faisait de toute façon avant
 * ce module.
 *
 * Le crénage et les ligatures sont coupés, comme dans `font.ts` : ce qu'on
 * montre est ce que le livre imprimera, et le navigateur crénerait une
 * ligne que le moteur ne crénera jamais.
 */
export function chargerApercu(rang: number): Promise<string | null> {
  const su = connus.get(rang);
  if (su) return Promise.resolve(su.famille);
  const encours = enCours.get(rang);
  if (encours) return encours;
  if (typeof document === "undefined" || depense >= BUDGET) {
    connus.set(rang, { famille: null });
    return Promise.resolve(null);
  }

  const p = (async () => {
    try {
      const octets = await policeApercu(rang, PLAFOND);
      const nom = famille(rang);
      const face = new FontFace(nom, octets, {
        featureSettings: '"liga" 0, "clig" 0, "kern" 0',
      });
      const posee = await face.load();
      document.fonts.add(posee);
      posees.set(rang, posee);
      depense += octets.byteLength;
      connus.set(rang, { famille: nom });
      return nom;
    } catch {
      connus.set(rang, { famille: null });
      return null;
    } finally {
      enCours.delete(rang);
    }
  })();
  enCours.set(rang, p);
  return p;
}

/**
 * Tout retirer.
 *
 * Le panneau se ferme, les spécimens partent avec lui : ils ne servent
 * qu'à lui, les rangs ne veulent plus rien dire dès que la liste est
 * relue, et une pile de faces qui grossit à chaque ouverture serait une
 * fuite tranquille. Rouvrir les redemande, ce qui coûte quelques
 * millisecondes par face visible.
 */
export function oublierApercus(): void {
  if (typeof document !== "undefined") {
    posees.forEach((f) => document.fonts.delete(f));
  }
  posees.clear();
  connus.clear();
  enCours.clear();
  depense = 0;
}
