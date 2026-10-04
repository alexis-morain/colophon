// Ce que la commande décide, hors du DOM (K-s3) : quand « Commander »
// s'affiche, comment un prix s'écrit, ce que dit un état, quand un devis a
// expiré, et la phrase de la confirmation.
//
// « Commander » n'apparaît jamais grisé. Sans clé, sans dépôt, sans dossier
// préparé dans cette ouverture d'Envoi ou sans contrôle vert sur ses
// fichiers, rien ne s'affiche : « Préparer » reste la porte, et l'écran
// n'annonce pas une commande intégrée à qui ne l'a pas réglée.
//
// Les secrets ne passent jamais par ici. La fenêtre reçoit une vue où la clé
// n'est plus que ses quatre derniers caractères (`VueCle.fin`).

import type { Preparation } from "./bridge";
import { Lang, t } from "./i18n";

/** Déclaré avec la clé : aucune réponse de l'API ne le dit. */
export type Mode = "Sandbox" | "Reel";

export type VueCle = { fin: string; mode: Mode };
export type VueDepot = { endpoint: string; region: string; bucket: string };
export type VueCommande = {
  reference: string;
  identifiant: string;
  mode: Mode;
  date: string;
  /** Le dernier numéro d'état lu, null tant que rien n'a été relu. */
  etat: number | null;
  objets_retires: boolean;
};
export type Vue = {
  cle: VueCle | null;
  depot: VueDepot | null;
  commandes: VueCommande[];
};

export type Offre = {
  quote: string;
  niveau: string;
  service: string | null;
  transporteur: string | null;
  prix: string;
  tva: string;
  devise: string;
};
export type Devis = {
  prix: string;
  tva: string;
  devise: string;
  expire_date: string;
  production_jours: number | null;
  expeditions: { poids_g: string | null; offres: Offre[] }[];
};

/** Le papier et la finition présélectionnés (décision 4), quand le produit
 *  les propose. */
export const PAPIER_DEFAUT = "pageblock_150mcs";
export const FINITION_DEFAUT = "cover_finish_matte";

/** « Commander » s'affiche-t-il ? La clé et le dépôt enregistrés, un dossier
 *  préparé dans cette ouverture d'Envoi, et le contrôle relu sur ses
 *  fichiers vert. `preparationNeuve` est nulle quand le dossier date d'une
 *  ouverture précédente. */
export function commandePossible(
  vue: Vue | null,
  preparationNeuve: Preparation | null,
): boolean {
  return !!vue?.cle && !!vue.depot && !!preparationNeuve && preparationNeuve.rapport.ok;
}

/** Un montant de l'API (« 11.8333 ») en dix-millièmes, sans flottant qui
 *  dérive dans une somme. */
function enUnites(montant: string): number {
  const [ent, dec = ""] = montant.trim().split(".");
  const signe = ent.startsWith("-") ? -1 : 1;
  return signe * (Math.abs(Number(ent)) * 10000 + Number((dec + "0000").slice(0, 4)));
}

/** Le total TTC d'un devis avec l'offre d'expédition choisie : articles, TVA
 *  des articles, expédition et sa TVA. */
export function totalTTC(devis: Devis, offre: Offre): string {
  const u =
    enUnites(devis.prix) + enUnites(devis.tva) + enUnites(offre.prix) + enUnites(offre.tva);
  return (u / 10000).toFixed(4);
}

/** Un montant arrondi au centime, dans la langue de l'écran. */
export function formatPrix(montant: string, devise: string, langue: Lang): string {
  return new Intl.NumberFormat(langue === "fr" ? "fr-FR" : "en-GB", {
    style: "currency",
    currency: devise,
  })
    .format(Math.round(enUnites(montant) / 100) / 100)
    .replace(/ | /g, " ");
}

/** Le devis a-t-il expiré à `maintenant` ? Une date illisible vaut une
 *  date passée : un devis douteux se refait. */
export function devisExpire(expire_date: string, maintenant: Date): boolean {
  const d = Date.parse(expire_date);
  return Number.isNaN(d) || d <= maintenant.getTime();
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

/** « Annuler » tant que la production n'a pas commencé : sous 30. Un état
 *  jamais lu laisse essayer, Cloudprinter répond 409 s'il est trop tard. */
export function annulable(code: number | null): boolean {
  return code === null || code < 30;
}

/** La confirmation : le prix et le mode en toutes lettres. */
export function phraseConfirmation(prix: string, mode: Mode): string {
  return t(mode === "Sandbox" ? "commande.confirmer.sandbox" : "commande.confirmer.reel", {
    prix,
  });
}

/** Une erreur de commande, dite. Deux codes du moteur ont leur phrase ; le
 *  reste voyage tel quel. */
export function phraseErreur(e: unknown): string {
  const s = String(e);
  if (s.includes("trop_tard")) return t("commande.annuler.tard");
  if (s.includes("devis_expire")) return t("commande.devis.expire");
  return t("commande.erreur", { erreur: s.replace(/^Error: /, "") });
}
