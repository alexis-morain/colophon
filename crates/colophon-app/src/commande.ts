// Ce que la commande décidera, hors du DOM : quand « Commander » s'affiche,
// comment un prix s'écrit, ce que dit un état, la phrase de la confirmation
// et celle d'une erreur.
//
// État présent : aucun relais n'existe, et l'app n'offre pas Commander.
// Aucun écran n'appelle ces fonctions ; elles attendent le parcours de S-s3,
// qui parlera au relais. L'app ne tient ni clé ni accès à un dépôt.
//
// « Commander » n'apparaîtra jamais grisé. Sans relais joignable, sans
// dossier préparé dans cette ouverture d'Envoi ou sans contrôle vert sur ses
// fichiers, rien ne s'affiche : « Préparer » reste la porte.

import type { Preparation } from "./bridge";
import { Lang, t } from "./i18n";

/** Sandbox ou réel : le relais le déclare dans sa réponse. */
export type Mode = "Sandbox" | "Reel";

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
    .replace(/ | /g, " ");
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
