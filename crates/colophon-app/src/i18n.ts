// Les deux langues de l'application, sans bibliothèque.
//
// Une dépendance de plus pour choisir entre deux dictionnaires serait payée
// par tout le monde et lue par personne : `t()` est une recherche dans un
// objet, la langue vit dans un `localStorage`, et le changement de langue est
// un rendu React, pas un redémarrage.
//
// **Ce qui n'est pas traduit, et c'est un arbitrage écrit** : la CLI et les
// messages du moteur restent en anglais. Le moteur parle à un développeur et
// à un fichier de log ; l'application parle à quelqu'un qui fabrique un
// album. Quand une commande Rust échoue, l'application fournit la phrase
// humaine (traduite) et le message du moteur voyage derrière, tel quel.
//
// Les clés sont nommées par écran (`bar.*`, `envoi.*`) : une clé orpheline se
// voit, et le test de parité ci-dessous refuse qu'une des deux langues ait
// une entrée que l'autre n'a pas.

import { useSyncExternalStore } from "react";

export type Lang = "fr" | "en";

const CLE = "colophon.langue";

/** Le français d'abord : c'est la langue dans laquelle tout a été écrit, et
 *  celle qui sert de référence quand les deux divergent. */
export const FR = {
  // -- la barre et les vues
  "bar.titre.aria": "Titre de l’album",
  "bar.livre": "Livre",
  "bar.tri": "Tri",
  "bar.planches": "Planches",
  "bar.envoi": "Envoi",
  "bar.annuler": "Annuler",
  "bar.retablir": "Rétablir",
  "bar.enregistrer": "Enregistrer",
  "bar.recomposer": "Recomposer",
  "bar.nouveau": "Nouveau",
  "bar.ouvrir": "Ouvrir",
  "bar.envoi.titre": "⌘4 · le contrôle avant impression",
  "bar.nouveau.titre": "Fermer et composer un autre album",

  // -- l'écran d'accueil
  "accueil.kicker": "Colophon",
  "accueil.titre": "Un dossier de photos,\nun album à feuilleter.",
  "accueil.lede":
    "Colophon lit vos photos, écarte les doublons et les ratés, compose les planches et rend un PDF prêt à relire. Tout se retouche ensuite : gabarits, ordre, photos repêchées.",
  "accueil.choisir": "Choisir un dossier de photos…",
  "accueil.ou.ouvrir": "ou Ouvrir un album existant",
  "accueil.recents": "Albums récents",

  // -- le panneau de stockage
  "stockage.titre": "Stockage",
  "stockage.fermer": "Fermer (Échap)",
  "stockage.mesure": "Mesure du dossier de données…",
  "stockage.total.suite":
    "sur ce disque, {n} albums. Les photos d’origine ne sont pas comptées ici : Colophon ne les copie jamais.",
  "stockage.total.suite.un":
    "sur ce disque, 1 album. Les photos d’origine ne sont pas comptées ici : Colophon ne les copie jamais.",
  "stockage.ouvert": "ouvert",
  "stockage.supprimer": "Supprimer",
  "stockage.vide": "Aucun album composé sur cette machine.",
  "stockage.repartition": "vignettes {vignettes}, aperçu {apercu}",
  "stockage.purger": "Vider les caches de vignettes ({poids})",
  "stockage.ouvrir.dossier": "Ouvrir le dossier",
  "stockage.note":
    "Un album supprimé ne se récupère pas. Le dossier de photos, lui, reste intact : rien ici n’écrit ou n’efface hors du dossier de Colophon.",
  "stockage.planches": "{n} planches",
  "stockage.date.inconnue": "date inconnue",
  "stockage.confirme.supprimer":
    "Supprimer « {titre} » ?\n\nCela libère {poids} et efface la composition : les planches, les recadrages, les légendes et l’aperçu.\n\nVos photos ne sont pas touchées, elles restent dans leur dossier.",
  "stockage.confirme.purger":
    "Vider les caches de vignettes ?\n\nCela libère {poids}. Aucun album n’est perdu : les vignettes se reconstruisent à la prochaine ouverture, ce qui prend quelques secondes par album.",
  "stockage.supprime": "« {titre} » supprimé, {poids} libérés.",
  "stockage.purge": "Caches vidés, {poids} libérés.",

  // -- À propos
  "apropos.titre": "À propos de Colophon",
  "apropos.version": "Version {version}, sous licence",
  "apropos.licence": "GNU General Public License v3.0",
  "apropos.quoi":
    "Un dossier de photos en entrée, un album composé, tout modifiable, un PDF prêt à imprimer. Le code source est public : vous pouvez le lire, le modifier et le redistribuer aux mêmes conditions.",
  "apropos.actifs": "Ce qui voyage à l’intérieur",
  "apropos.police.quoi":
    "La police du livre et de l’interface. C’est elle que le PDF incorpore.",
  "apropos.icc.quoi": "Le profil couleur que le PDF embarque comme OutputIntent.",
  "apropos.geonames.quoi":
    "Les noms de villes qui titrent les chapitres, depuis le GPS des photos.",
  "apropos.notices.voir": "Notices des licences tierces",
  "apropos.notices.masquer": "Masquer les notices des licences tierces",
  "apropos.notices.absentes":
    "Les notices n’ont pas été générées pour cette version (scripts/notices.sh).",

  // -- l'aperçu fidèle
  "fidele.voir": "Voir le PDF",
  "fidele.actif": "Aperçu fidèle",
  "fidele.titre.on":
    "Afficher la page telle que le PDF la contient, rendue par pdf.js (⇧⌘P)",
  "fidele.titre.off": "Retour au rendu de l’éditeur, celui qu’on peut modifier (⇧⌘P)",
  "fidele.rendu": "Rendu de l’aperçu fidèle…",
  "fidele.pret": "Aperçu fidèle : ce que la presse recevra, au pixel près",
  "fidele.harnais": "Aperçu fidèle : le PDF du dossier de dev, pas forcément à jour",
  "fidele.aria": "Aperçu fidèle, rendu depuis le PDF",

  // -- la mise à jour
  "maj.dispo": "Colophon {version} est disponible.",
  "maj.attente": " Le téléchargement et le redémarrage prennent une minute.",
  "maj.encours": " Téléchargement en cours, l’app redémarrera toute seule.",
  "maj.installer": "Installer maintenant",
  "maj.installation": "Installation…",
  "maj.plus.tard": "Plus tard",

  // -- les préférences
  "prefs.titre": "Préférences",
  "prefs.langue": "Langue de l’application",
  "prefs.langue.note":
    "La ligne de commande et les messages du moteur restent en anglais : ils parlent à un développeur, pas à quelqu’un qui fabrique un album.",
  "prefs.theme": "Apparence",
  "prefs.theme.note":
    "Colophon suit le réglage du système. Le papier des planches, lui, ne change jamais de teinte : c’est du papier.",

  // -- messages d'état et erreurs
  "etat.enregistre": "Enregistré",
  "etat.titre.modifie": "Titre modifié : ⌘S l’enregistre",
  "etat.colophon.ajoute":
    "Page de colophon ajoutée : ⌘S l’enregistre, le prévol recompte les pages",
  "etat.colophon.retire": "Page de colophon retirée",
  "etat.colophon.trop.vieux":
    "Cet album a été composé avant la page de colophon : recomposez-le pour l’obtenir",
  "etat.garde.ajoutee":
    "Page de garde ajoutée : ⌘S l’enregistre, le prévol recompte les pages",
  "etat.garde.retiree": "Page de garde retirée",
  "etat.garde.trop.vieux":
    "Cet album a été composé avant la page de garde : recomposez-le pour l’obtenir",
  "etat.auto.rendue": "Planche {n} rendue à l’automatique (⌘Z la ramène)",
  "etat.auto.insertion":
    "Planche {n} : insérée à la main, elle n’a pas de version automatique",
  "erreur.enregistrement": "L’enregistrement a échoué : rien n’a été écrit.",
  "erreur.ouverture": "L’album n’a pas pu être ouvert.",
  "erreur.reouverture": "Cet album n’a pas pu être rouvert. A-t-il été déplacé ?",
  "erreur.auto": "Cette planche n’a pas pu être rendue à l’automatique.",
  "erreur.colophon": "La page de colophon n’a pas pu être changée.",
  "erreur.garde": "La page de garde n’a pas pu être changée.",
  "erreur.fidele": "L’aperçu fidèle n’a pas pu être rendu.",
  "erreur.variante": "Cette proposition n’a pas pu être ouverte.",
  "erreur.maj": "La mise à jour n’a pas pu être installée.",
} as const;

export type Cle = keyof typeof FR;

export const EN: Record<Cle, string> = {
  "bar.titre.aria": "Album title",
  "bar.livre": "Book",
  "bar.tri": "Sort",
  "bar.planches": "Spreads",
  "bar.envoi": "Send",
  "bar.annuler": "Undo",
  "bar.retablir": "Redo",
  "bar.enregistrer": "Save",
  "bar.recomposer": "Recompose",
  "bar.nouveau": "New",
  "bar.ouvrir": "Open",
  "bar.envoi.titre": "⌘4 · the check before printing",
  "bar.nouveau.titre": "Close and compose another album",

  "accueil.kicker": "Colophon",
  "accueil.titre": "A folder of photographs,\na book to leaf through.",
  "accueil.lede":
    "Colophon reads your photographs, sets aside the duplicates and the misses, lays out every spread and hands you a PDF to argue with. Everything can be changed afterwards: templates, order, rescued photos.",
  "accueil.choisir": "Choose a folder of photographs…",
  "accueil.ou.ouvrir": "or Open an existing album",
  "accueil.recents": "Recent albums",

  "stockage.titre": "Storage",
  "stockage.fermer": "Close (Esc)",
  "stockage.mesure": "Measuring the data folder…",
  "stockage.total.suite":
    "on this disk, {n} albums. Your original photographs are not counted here: Colophon never copies them.",
  "stockage.total.suite.un":
    "on this disk, 1 album. Your original photographs are not counted here: Colophon never copies them.",
  "stockage.ouvert": "open",
  "stockage.supprimer": "Delete",
  "stockage.vide": "No album composed on this machine.",
  "stockage.repartition": "thumbnails {vignettes}, preview {apercu}",
  "stockage.purger": "Empty the thumbnail caches ({poids})",
  "stockage.ouvrir.dossier": "Open the folder",
  "stockage.note":
    "A deleted album does not come back. The photo folder stays untouched: nothing here writes or erases outside Colophon’s own folder.",
  "stockage.planches": "{n} spreads",
  "stockage.date.inconnue": "date unknown",
  "stockage.confirme.supprimer":
    "Delete “{titre}”?\n\nThis frees {poids} and erases the composition: the spreads, the crops, the captions and the preview.\n\nYour photographs are not touched, they stay in their folder.",
  "stockage.confirme.purger":
    "Empty the thumbnail caches?\n\nThis frees {poids}. No album is lost: the thumbnails rebuild themselves at the next open, which takes a few seconds per album.",
  "stockage.supprime": "“{titre}” deleted, {poids} freed.",
  "stockage.purge": "Caches emptied, {poids} freed.",

  "apropos.titre": "About Colophon",
  "apropos.version": "Version {version}, under the",
  "apropos.licence": "GNU General Public License v3.0",
  "apropos.quoi":
    "A folder of photographs in, a composed album, everything editable, a print-ready PDF. The source code is public: you may read it, change it and redistribute it under the same terms.",
  "apropos.actifs": "What travels inside",
  "apropos.police.quoi":
    "The face of the book and of the interface. It is the one the PDF embeds.",
  "apropos.icc.quoi": "The colour profile the PDF carries as its OutputIntent.",
  "apropos.geonames.quoi":
    "The town names that title the chapters, from the GPS your cameras wrote.",
  "apropos.notices.voir": "Third-party licence notices",
  "apropos.notices.masquer": "Hide the third-party licence notices",
  "apropos.notices.absentes":
    "The notices were not generated for this version (scripts/notices.sh).",

  "fidele.voir": "See the PDF",
  "fidele.actif": "Faithful preview",
  "fidele.titre.on":
    "Show the page as the PDF holds it, rendered by pdf.js (⇧⌘P)",
  "fidele.titre.off": "Back to the editor’s own rendering, the one you can change (⇧⌘P)",
  "fidele.rendu": "Rendering the faithful preview…",
  "fidele.pret": "Faithful preview: what the press will receive, to the pixel",
  "fidele.harnais": "Faithful preview: the dev folder’s PDF, not necessarily current",
  "fidele.aria": "Faithful preview, rendered from the PDF",

  "maj.dispo": "Colophon {version} is available.",
  "maj.attente": " Downloading and restarting take about a minute.",
  "maj.encours": " Downloading; the app will restart on its own.",
  "maj.installer": "Install now",
  "maj.installation": "Installing…",
  "maj.plus.tard": "Later",

  "prefs.titre": "Preferences",
  "prefs.langue": "Application language",
  "prefs.langue.note":
    "The command line and the engine’s messages stay in English: they speak to a developer, not to somebody making an album.",
  "prefs.theme": "Appearance",
  "prefs.theme.note":
    "Colophon follows the system setting. The paper of the spreads never changes shade: it is paper.",

  "etat.enregistre": "Saved",
  "etat.titre.modifie": "Title changed: ⌘S saves it",
  "etat.colophon.ajoute":
    "Colophon page added: ⌘S saves it, the preflight recounts the pages",
  "etat.colophon.retire": "Colophon page removed",
  "etat.colophon.trop.vieux":
    "This album was composed before the colophon page existed: recompose it to get one",
  "etat.garde.ajoutee":
    "Half-title added: ⌘S saves it, the preflight recounts the pages",
  "etat.garde.retiree": "Half-title removed",
  "etat.garde.trop.vieux":
    "This album was composed before the half-title existed: recompose it to get one",
  "etat.auto.rendue": "Spread {n} given back to the machine (⌘Z brings it back)",
  "etat.auto.insertion":
    "Spread {n}: inserted by hand, it has no automatic version",
  "erreur.enregistrement": "Saving failed: nothing was written.",
  "erreur.ouverture": "The album could not be opened.",
  "erreur.reouverture": "This album could not be reopened. Has it moved?",
  "erreur.auto": "This spread could not be given back to the machine.",
  "erreur.colophon": "The colophon page could not be changed.",
  "erreur.garde": "The half-title could not be changed.",
  "erreur.fidele": "The faithful preview could not be rendered.",
  "erreur.variante": "This proposal could not be opened.",
  "erreur.maj": "The update could not be installed.",
};

const DICOS: Record<Lang, Record<Cle, string>> = { fr: FR, en: EN };

/** La langue du système quand personne n'a choisi. Tout ce qui n'est pas
 *  français part en anglais : ce sont les deux seules langues écrites, et
 *  proposer du français à quelqu'un qui n'en lit pas serait pire que
 *  l'anglais, que presque tout le monde déchiffre. */
function langueParDefaut(): Lang {
  const nav = typeof navigator === "undefined" ? "" : navigator.language;
  return nav.toLowerCase().startsWith("fr") ? "fr" : "en";
}

let courante: Lang = (() => {
  try {
    const gardee =
      typeof localStorage === "undefined" ? null : localStorage.getItem(CLE);
    if (gardee === "fr" || gardee === "en") return gardee;
  } catch {
    /* un stockage bloqué ne coûte que la mémoire du choix */
  }
  return langueParDefaut();
})();

const abonnes = new Set<() => void>();

export function langue(): Lang {
  return courante;
}

/** Changer de langue est un rendu, jamais un redémarrage : tout le monde a
 *  déjà vu une application demander à redémarrer pour ça. */
export function setLangue(l: Lang) {
  if (l === courante) return;
  courante = l;
  try {
    if (typeof localStorage !== "undefined") localStorage.setItem(CLE, l);
  } catch {
    /* idem */
  }
  // L'attribut `lang` du document sert à la coupure de mots et aux lecteurs
  // d'écran. Absent des tests, qui tournent sans DOM.
  if (typeof document !== "undefined") document.documentElement.lang = l;
  abonnes.forEach((f) => f());
}

/** Rend le composant appelant sensible au changement de langue. */
export function useLangue(): Lang {
  return useSyncExternalStore(
    (f) => {
      abonnes.add(f);
      return () => abonnes.delete(f);
    },
    () => courante,
    () => courante,
  );
}

/**
 * Le texte d'une clé, avec ses trous remplis. Une clé absente rend la clé
 * elle-même : un écran qui affiche `envoi.verdict` est un écran dont le
 * défaut se voit, là où une chaîne vide passerait inaperçue jusqu'à
 * l'impression.
 */
export function t(cle: Cle, params?: Record<string, string | number>): string {
  const texte = DICOS[courante][cle] ?? FR[cle] ?? cle;
  if (!params) return texte;
  return texte.replace(/\{(\w+)\}/g, (tel, nom) =>
    nom in params ? String(params[nom]) : tel,
  );
}
