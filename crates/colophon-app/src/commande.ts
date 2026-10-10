// Ce que Commander décide, hors du DOM : quand le bouton s'affiche, comment
// un prix s'écrit, ce que dit chaque état du relais, et la machine du
// parcours (pays, grille, envoi, confirmation, paiement, attente).
//
// L'app parle au relais d'Alexis par le moteur (S-s3), jamais par le
// webview : le secret de chaque intention reste dans `commande.json`, la
// fenêtre ne reçoit qu'identifiant, mode, état et dernier code, plus la
// grille des prix.
//
// « Commander » n'apparaît jamais grisé. Sans relais joignable (un build de
// release n'en porte aucun tant que le relais réel n'existe pas), sans
// dossier préparé dans cette ouverture d'Envoi ou sans contrôle vert sur ses
// fichiers, rien ne s'affiche : « Préparer » reste la porte.

import type { Preparation } from "./bridge";
import { Cle, Lang, t } from "./i18n";

/** Sandbox ou réel : le relais le déclare dans chaque réponse. */
export type Mode = "sandbox" | "reel";

/** Un État ou une région, quand l'adresse en exige un. */
export type EtatRegion = { code: string; nom: string };

/** Un pays ouvert à la commande, tel que le relais le rend. */
export type Pays = { code: string; nom: string; require_state: boolean; etats?: EtatRegion[] };

/** Un niveau d'expédition : le prix de vente TTC, livraison comprise. */
export type Niveau = {
  niveau: string;
  transporteur: string | null;
  prix_ttc_centimes: number;
  delai_jours: number | null;
};

/** Une intention créée : sa grille, sans secret ni URL de dépôt. */
export type Offre = { id: string; mode: Mode; grille: Niveau[]; expire: string };

/** Une intention gardée, telle que la fenêtre la voit. */
export type IntentionVue = {
  id: string;
  mode: Mode;
  etat: string;
  dernier_code: number | null;
  /** Le niveau d'expédition choisi, gardé dès le choix. */
  niveau: string | null;
};

export type Quel = "interieur" | "couverture";

/** Ce que l'envoi émet : le fichier, les octets partis, le total. */
export type Progres = { fichier: Quel; envoyes: number; total: number };

/** « Commander » s'affiche-t-il ? Le relais joignable, un dossier préparé
 *  dans cette ouverture d'Envoi, et le contrôle relu sur ses fichiers vert.
 *  `preparationNeuve` est nulle quand le dossier date d'une ouverture
 *  précédente. */
export function commandePossible(
  relaisJoignable: boolean,
  preparationNeuve: Preparation | null,
): boolean {
  return relaisJoignable && !!preparationNeuve && preparationNeuve.rapport.ok;
}

/** Un montant de l'API (« 11.8333 ») en dix-millièmes, sans flottant qui
 *  dérive dans une somme. */
function enUnites(montant: string): number {
  const [ent, dec = ""] = montant.trim().split(".");
  const signe = ent.startsWith("-") ? -1 : 1;
  return signe * (Math.abs(Number(ent)) * 10000 + Number((dec + "0000").slice(0, 4)));
}

/** Un montant arrondi au centime, dans la langue de l'écran. */
export function formatPrix(montant: string, devise: string, langue: Lang): string {
  return new Intl.NumberFormat(langue === "fr" ? "fr-FR" : "en-GB", {
    style: "currency",
    currency: devise,
  })
    .format(Math.round(enUnites(montant) / 100) / 100)
    .replace(/\u202f|\u00a0/g, " ");
}

/** Un prix de la grille, en centimes entiers, écrit en euros. */
export function prixDeCentimes(centimes: number, langue: Lang): string {
  const c = Math.round(centimes);
  return formatPrix(`${Math.floor(c / 100)}.${String(c % 100).padStart(2, "0")}`, "EUR", langue);
}

/** Les niveaux d'expédition que Cloudprinter nomme. Un niveau inconnu
 *  garde son code : le relais le rend tel quel. */
const NIVEAUX: Record<string, Cle> = {
  cp_postal: "commande.niveau.cp_postal",
  cp_ground: "commande.niveau.cp_ground",
  cp_saver: "commande.niveau.cp_saver",
  cp_fast: "commande.niveau.cp_fast",
};

/** Le nom d'un niveau, puis le transporteur quand le relais le dit. */
export function nomNiveau(n: Niveau): string {
  const nom = Object.prototype.hasOwnProperty.call(NIVEAUX, n.niveau) ? t(NIVEAUX[n.niveau]) : n.niveau;
  return n.transporteur ? `${nom} · ${n.transporteur}` : nom;
}

/** Une phrase par état, du numéro que `orders/info` rend dans `state`. */
export function phraseEtat(code: number | null): string {
  if (code === null) return t("commande.etat.aucun");
  if (code === 1) return t("commande.etat.nouvelle");
  if (code >= 5 && code <= 7) return t("commande.etat.verification");
  if (code >= 10 && code <= 15) return t("commande.etat.fichiers");
  if (code >= 30 && code <= 45) return t("commande.etat.rendu");
  if (code === 100) return t("commande.etat.expediee");
  if (code === 500) return t("commande.etat.annulee");
  if (code === 501) return t("commande.etat.sandbox");
  return t("commande.etat.inconnu", { code });
}

/** Les neuf états du relais. */
const ETATS: Record<string, Cle> = {
  attente_fichiers: "commande.relais.attente_fichiers",
  attente_paiement: "commande.relais.attente_paiement",
  payee: "commande.relais.payee",
  commandee: "commande.relais.commandee",
  remboursement_en_cours: "commande.relais.remboursement_en_cours",
  echec_remboursee: "commande.relais.echec_remboursee",
  annulation_en_cours: "commande.relais.annulation_en_cours",
  annulee: "commande.relais.annulee",
  purgee: "commande.relais.purgee",
};

/** La phrase d'une intention : son état au relais, et, une fois commandée,
 *  sa référence et l'état chez Cloudprinter. Un état que l'app ne connaît
 *  pas a une phrase neutre, jamais une exception. */
export function phraseIntention(i: IntentionVue): string {
  if (!Object.prototype.hasOwnProperty.call(ETATS, i.etat)) return t("commande.relais.inconnu", { etat: i.etat });
  const phrase = t(ETATS[i.etat], { reference: i.id });
  return i.etat === "commandee" ? `${phrase} ${phraseEtat(i.dernier_code)}` : phrase;
}

/** « Annuler » tant que la production n'a pas commencé : sous 30. Un état
 *  jamais lu laisse essayer, Cloudprinter répond 409 s'il est trop tard. */
export function annulable(code: number | null): boolean {
  return code === null || code < 30;
}

/** « Annuler » s'offre sur une commande passée, sous 30. */
export function annulableIntention(i: IntentionVue): boolean {
  return i.etat === "commandee" && annulable(i.dernier_code);
}

/** Les états qui bougent seuls dans les secondes qui viennent : l'écran les
 *  relit toutes les cinq secondes tant qu'il est ouvert. */
export function enAttente(etat: string): boolean {
  return ["attente_paiement", "payee", "remboursement_en_cours", "annulation_en_cours"].includes(etat);
}

/** Les intentions à relire maintenant. */
export function aRelire(intentions: IntentionVue[]): string[] {
  return intentions.filter((i) => enAttente(i.etat)).map((i) => i.id);
}

/** Les intentions que l'écran liste : toutes, sauf celle que le parcours
 *  tient en ce moment (`enCours`), qui se montre déjà là-haut. */
export function visibles(intentions: IntentionVue[], enCours: string | null): IntentionVue[] {
  return intentions.filter((i) => i.id !== enCours);
}

/** « Payer » s'offre dans la liste sur une intention qui attend son
 *  paiement et dont on connaît le niveau choisi. */
export function payable(i: IntentionVue): boolean {
  return i.etat === "attente_paiement" && !!i.niveau;
}

/** « Abandonner » s'offre tant que rien n'est payé. L'intention s'oublie
 *  localement ; le relais la purge à 24 h. */
export function abandonnable(i: IntentionVue): boolean {
  return i.etat === "attente_fichiers" || i.etat === "attente_paiement";
}

/** La confirmation : le prix et le mode en toutes lettres. */
export function phraseConfirmation(prix: string, mode: Mode): string {
  return t(mode === "sandbox" ? "commande.confirmer.sandbox" : "commande.confirmer.reel", {
    prix,
  });
}

/** Les codes du relais qui ont leur phrase ; le reste voyage tel quel. */
const ERREURS: [string, Cle][] = [
  ["trop_tard", "commande.annuler.tard"],
  ["devis_expire", "commande.devis.expire"],
  ["intention_expiree", "commande.devis.expire"],
  ["trop_de_sessions", "commande.sessions"],
  ["commander_indisponible", "commande.indisponible"],
  ["trop_de_demandes", "commande.indisponible"],
  ["autre papier", "commande.papier"],
];

/** Un refus du paiement qui ne se rattrape pas en réessayant : le parcours
 *  doit recommencer. */
export function refusDefinitif(e: unknown): boolean {
  const s = String(e);
  return ["devis_expire", "intention_expiree", "trop_de_sessions"].some((c) => s.includes(c));
}

/** Le second envoi d'une intention dont les fichiers partent déjà : le
 *  moteur le refuse sans rien toucher, et l'écran n'en dit rien. */
export function ignorable(e: unknown): boolean {
  return String(e).includes("envoi_en_cours");
}

/** Une erreur de commande, dite. */
export function phraseErreur(e: unknown): string {
  const s = String(e);
  for (const [code, cle] of ERREURS) if (s.includes(code)) return t(cle);
  return t("commande.erreur", { erreur: s.replace(/^Error: /, "") });
}

// ---- le parcours ------------------------------------------------------------

/** Où en est le parcours de Commander. */
export type Parcours =
  | { etape: "repos" }
  | { etape: "pays"; pays: Pays[]; choisi: string; region: string | null }
  | { etape: "grille"; offre: Offre; niveau: string }
  | { etape: "envoi"; offre: Offre; niveau: string; progres: Record<Quel, { envoyes: number; total: number }> }
  | { etape: "confirmation"; offre: Offre; niveau: string }
  | { etape: "pret"; offre: Offre; niveau: string }
  | { etape: "paiement"; offre: Offre; niveau: string }
  | { etape: "attente"; offre: Offre; niveau: string }
  | { etape: "erreur"; phrase: string };

export type Evenement =
  | { type: "commander"; pays: Pays[] }
  | { type: "pays"; code: string }
  | { type: "region"; code: string }
  | { type: "offre"; offre: Offre }
  | { type: "niveau"; niveau: string }
  | { type: "envoyer" }
  | { type: "progres"; progres: Progres }
  | { type: "envoye" }
  | { type: "confirmer" }
  | { type: "payer" }
  | { type: "ouvert" }
  | { type: "echec"; definitif?: string }
  | { type: "recommencer"; pays: Pays[] }
  | { type: "retour" }
  | { type: "etat"; intention: IntentionVue };

const SANS_PROGRES = { envoyes: 0, total: 0 };

function paysChoisi(pays: Pays[], code: string): { choisi: string; region: string | null } {
  const p = pays.find((x) => x.code === code);
  return { choisi: code, region: p?.require_state ? (p.etats?.[0]?.code ?? null) : null };
}

/** La machine du parcours. Un événement qui ne concerne pas l'étape ne
 *  change rien : l'écran peut en recevoir un en retard sans casser. */
export function avancer(p: Parcours, e: Evenement): Parcours {
  switch (e.type) {
    case "recommencer":
      return p.etape === "erreur" ? avancer({ etape: "repos" }, { type: "commander", pays: e.pays }) : p;
    case "commander": {
      if (p.etape !== "repos" || e.pays.length === 0) return p;
      // La France d'abord quand elle est ouverte, sinon le premier.
      const code = e.pays.some((x) => x.code === "FR") ? "FR" : e.pays[0].code;
      return { etape: "pays", pays: e.pays, ...paysChoisi(e.pays, code) };
    }
    case "pays":
      return p.etape === "pays" && p.pays.some((x) => x.code === e.code)
        ? { ...p, ...paysChoisi(p.pays, e.code) }
        : p;
    case "region":
      return p.etape === "pays" ? { ...p, region: e.code } : p;
    case "offre":
      return p.etape === "pays" && e.offre.grille.length > 0
        ? { etape: "grille", offre: e.offre, niveau: e.offre.grille[0].niveau }
        : p;
    case "niveau":
      return p.etape === "grille" && p.offre.grille.some((n) => n.niveau === e.niveau)
        ? { ...p, niveau: e.niveau }
        : p;
    case "envoyer":
      return p.etape === "grille"
        ? { etape: "envoi", offre: p.offre, niveau: p.niveau, progres: { interieur: SANS_PROGRES, couverture: SANS_PROGRES } }
        : p;
    case "progres":
      return p.etape === "envoi"
        ? { ...p, progres: { ...p.progres, [e.progres.fichier]: { envoyes: e.progres.envoyes, total: e.progres.total } } }
        : p;
    case "envoye":
      return p.etape === "envoi" ? { etape: "confirmation", offre: p.offre, niveau: p.niveau } : p;
    case "confirmer":
      return p.etape === "pret" ? { etape: "confirmation", offre: p.offre, niveau: p.niveau } : p;
    case "payer":
      return p.etape === "confirmation" || p.etape === "attente"
        ? { etape: "paiement", offre: p.offre, niveau: p.niveau }
        : p;
    case "ouvert":
      return p.etape === "paiement" ? { etape: "attente", offre: p.offre, niveau: p.niveau } : p;
    case "echec":
      // L'envoi raté revient à la grille, où « Envoyer » réessaie ; un
      // paiement raté revient au bouton qui le rouvre.
      // Un refus définitif (`refusDefinitif`) mène à l'étape d'erreur, d'où
      // l'on recommence.
      if (p.etape === "envoi") return { etape: "grille", offre: p.offre, niveau: p.niveau };
      if (p.etape === "paiement") {
        return e.definitif !== undefined
          ? { etape: "erreur", phrase: e.definitif }
          : { etape: "pret", offre: p.offre, niveau: p.niveau };
      }
      return p;
    case "retour":
      // Les fichiers partis restent partis : refermer la confirmation garde
      // le bouton qui la rouvre. Avant l'envoi, on repart de zéro ; pendant,
      // rien ne s'interrompt.
      if (p.etape === "confirmation") return { etape: "pret", offre: p.offre, niveau: p.niveau };
      return ["pays", "grille", "pret", "erreur"].includes(p.etape) ? { etape: "repos" } : p;
    case "etat":
      // Le paiement a abouti ou échoué : la liste des commandes prend le
      // relais, le parcours se referme.
      return p.etape === "attente" && e.intention.id === p.offre.id && e.intention.etat !== "attente_paiement"
        ? { etape: "repos" }
        : p;
  }
}

/** Le niveau choisi dans la grille d'un parcours. */
export function niveauChoisi(p: { offre: Offre; niveau: string }): Niveau {
  return p.offre.grille.find((n) => n.niveau === p.niveau) ?? p.offre.grille[0];
}

/** La part d'un fichier déjà partie, de 0 à 1. */
export function part(x: { envoyes: number; total: number }): number {
  return x.total > 0 ? Math.min(1, x.envoyes / x.total) : 0;
}
