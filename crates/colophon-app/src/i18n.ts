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

  // -- commun aux panneaux
  "commun.fermer": "Fermer (Échap)",

  // -- le panneau de stockage
  "stockage.titre": "Stockage",
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
  "apropos.attribution":
    "Noms de lieux : données GeoNames (https://www.geonames.org), sous licence Creative Commons Attribution 4.0.",
  "apropos.icc.licence":
    "International Color Consortium, redistribution sans restriction",

  // -- le panneau Signaler
  "signaler.titre.bug": "Signaler un problème",
  "signaler.titre.planche": "Signaler une planche ratée",
  "signaler.titre.recadrage": "Signaler un recadrage raté",
  "signaler.intro":
    "Le rapport ci-dessous est construit sur cette machine. Relisez-le : c’est tout ce qui part, rien d’autre. Des chiffres et des noms de fichiers, jamais un chemin, une coordonnée GPS ni une légende.",
  "signaler.construction":
    "Rapport en construction… L’audit relit chaque photo, quelques secondes sur un grand album.",
  "signaler.piece":
    "Je joindrai moi-même une image de la planche sur la page GitHub. Rien n’est téléversé d’ici.",
  "signaler.ouvrir": "Ouvrir l’issue GitHub pré-remplie",
  "signaler.copier": "Copier le rapport",
  "signaler.copie": "Rapport copié",
  "signaler.note.ouverte":
    "L’issue s’ouvre dans votre navigateur, le rapport déjà en place : relisez, complétez, publiez.",
  "signaler.note.copie":
    "Sans réseau ou sans compte GitHub : copiez le rapport, il se colle tel quel dans une issue ou un mail, plus tard.",

  // -- le contenu du rapport (relu par la personne qui l'envoie)
  "rapport.album": "Album : {w} × {h} mm, {n} planches",
  "rapport.planche":
    "Planche {n} sur {total}, gabarit {gabarit}, {photos}{edition}{figee} :",
  "rapport.photos": "{n} photos",
  "rapport.photos.une": "1 photo",
  "rapport.editee": ", éditée à la main",
  "rapport.figee": ", figée",
  "rapport.case": "  case {i} : {geo}, photo {nom}",
  "rapport.case.hors.gabarit": "case hors gabarit",
  "rapport.hors.gabarit": "hors gabarit",
  "rapport.paysage": "paysage",
  "rapport.portrait": "portrait",
  "rapport.carree": "carrée",
  "rapport.case.signalee":
    "Case signalée : case {i}, {geo}, photo {nom}, point focal {fx} ; {fy}, zoom {zoom}",
  "rapport.audit.indisponible":
    "Audit : indisponible (pas d'album ouvert, ou audit en échec)",
  "rapport.audit.rouges": "{n} compteurs au-dessus du seuil",
  "rapport.audit.rouge.un": "1 compteur au-dessus du seuil",
  "rapport.audit.verts": "tous les compteurs sous leur seuil",
  "rapport.audit": "Audit ({n} planches) : {verdict}",
  "rapport.audit.note": "note : {note}",
  "rapport.image":
    "Image de la planche : ajoutée à la main dans GitHub, volontairement.",
  "rapport.log":
    "Extrait du log (chemins déjà réduits aux noms de fichiers) :",

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
  "prefs.rendu": "Rendu des planches",
  "prefs.rendu.dom": "Éléments",
  "prefs.rendu.canvas": "Canvas",
  "prefs.rendu.note":
    "Les deux dessinent la même planche, à partir des mêmes objets ; ils ne diffèrent que par la façon dont elle arrive à l’écran. Le canvas est en cours d’évaluation : si votre machine peine à faire glisser un recadrage, essayez-le. Le PDF ne change jamais, quel que soit ce réglage.",

  // -- création, composition, formats
  "setup.nouvel": "Nouvel album",
  "setup.changer.dossier": "Changer de dossier",
  "setup.titre": "titre",
  "setup.planches": "planches",
  "setup.pages": "soit {n} pages",
  "setup.format": "format de page",
  "setup.rythme": "rythme",
  "setup.rythme.hint":
    "Le rythme se rejoue à chaque recomposition ; chaque planche reste modifiable une par une.",
  "setup.composer": "Composer l’album",
  "setup.annuler": "Annuler",
  "setup.cm.ouvert": "{cm} cm ouvert",
  "setup.la.page": "la page",
  "format.carre-21": "Carré 21 × 21",
  "format.carre-30": "Carré 30 × 30",
  "format.portrait-a4": "Portrait A4",
  "format.paysage-a4": "Paysage A4",
  "format.paysage-28x21": "Paysage 28 × 21",
  "format.portrait-20x25": "Portrait 20 × 25",
  "compo.recomposition": "Recomposition de « {titre} »",
  "compo.composition": "Composition de « {titre} »",
  "compo.album.defaut": "l’album",
  "compo.hint.recomp":
    "Les planches éditées à la main ou verrouillées sont conservées telles quelles.",
  "compo.hint.compo":
    "L’analyse des photos ne se fait qu’une fois : recomposer ce dossier sera bien plus rapide.",
  "compo.arreter.titre": "Arrête la composition ; rien n’est écrit",
  "compo.details": "Détails techniques",
  "compo.lecture": "lecture du dossier…",
  "compo.annulee": "Composition annulée",
  "compo.vide":
    "Ce dossier n’a donné aucune photo exploitable, rien n’a été créé. Choisissez un autre dossier, ou rouvrez celui-ci après y avoir ajouté des photos.",
  "erreur.compo": "La composition a échoué.",
  "recomp.confirme":
    "Recomposer l’album ? Les planches éditées à la main ou verrouillées sont conservées telles quelles, les autres sont recomposées. L’historique d’annulation repart de zéro.",
  "recomp.ok": "Album recomposé, planches éditées conservées",
  "recomp.annulee": "Recomposition annulée",
  "erreur.recomp": "La recomposition a échoué.",
  "fermer.confirme": "Des modifications ne sont pas enregistrées. Fermer quand même ?",
  "stage.lecture": "lecture du dossier",
  "stage.scan": "inventaire du dossier",
  "stage.analyse.n": "analyse des photos, {i} sur {n}",
  "stage.analyse": "analyse des photos",
  "stage.parasites": "écart des parasites",
  "stage.dedup": "déduplication des rafales",
  "stage.eclaircissage": "éclaircissage des doublons",
  "stage.chapitres": "découpage en chapitres",
  "stage.layout": "mise en page des planches",
  "stage.pinned": "planches éditées remises en place",
  "stage.curation": "journal de curation",
  "stage.pdf": "rendu du PDF",

  // -- l'export
  "export.rendu": "Rendu du PDF d’impression…",
  "export.progress": "Rendu à 300 dpi : {done}/{total} photos…",
  "export.enregistrement.annule": "Enregistrement annulé",
  "export.fichiers": "{n} fichiers enregistrés : {liste}",
  "export.pdf": "PDF enregistré : {nom}",
  "export.annule": "Export annulé, aucun fichier écrit",
  "export.annuler": "Annuler l’export",
  "erreur.export": "Le rendu du PDF a échoué.",

  // -- gestes sur les planches et les photos
  "repeche.place":
    "Aucune place autour de la planche {n} : libérez une case ou changez un gabarit",
  "repeche.ok": "Repêchée sur la planche {n}",
  "place.doublon": "Déjà sur cette planche : deux fois la même photo serait un doublon",
  "place.remplacee": "Photo placée · l’ancienne repart dans la réserve",
  "place.ok": "Photo placée",
  "planche.dupliquee": "Planche {n} dupliquée",
  "planche.liberee": "Planche libérée",
  "planche.figee.status": "Planche figée : elle survivra à toute recomposition",
  "auto.confirme":
    "Rendre la planche {n} à l’automatique ?\n\nElle reprend la composition proposée au départ. Le recadrage, les légendes et les photos changées à la main sur cette planche sont perdus, et le cadenas tombe.\n\n⌘Z revient en arrière.",
  "planche.vide.inseree": "Planche vide insérée : une respiration",
  "planche.texte.inseree":
    "Planche de texte insérée : double-clic pour l’ouvrir et écrire",
  "planche.supprimee": "Planche {n} supprimée (⌘Z la ramène)",
  "planche.deplacee": "Planche déplacée en position {n}",
  "signal.planche.dabord":
    "Ouvrez d’abord la planche à signaler (vue Livre ou Planches)",
  "signal.case.dabord":
    "Sélectionnez d’abord la case au recadrage raté (vue Livre)",
  "revue.terminee": "Revue terminée, chaque écart est vu",
  "move.pleine": "Planche {n} pleine : aucun gabarit n’accepte une photo de plus",
  "move.texte": "Planche {n} : une page de texte, une photo l’effacerait",
  "move.refuse": "Refusé : il faudrait sacrifier une autre photo de cette planche",
  "move.ok": "Photo envoyée sur la planche {n}",
  "zoom.remis": "Zoom remis au remplissage exact",
  "legende.posee": "Légende posée : « {texte} » (⌘Z la retire)",

  // -- la barre, la ligne de contexte, les pieds de vue
  "bar.recomposer.titre":
    "Recompose l’album ; les planches éditées ou verrouillées sont conservées",
  "contexte.couverture":
    "La couverture : titre et sous-titre en place, glissez la photo pour la recadrer. Le tiroir de photos revient sur les planches.",
  "contexte.recadrage":
    "Recadrage : glisser déplace, molette zoome, ⌥ affine, ⌫ retire la photo",
  "nav.precedente": "Planche précédente",
  "nav.suivante": "Planche suivante",
  "nav.aller": "Aller à une planche",
  "nav.planche": "planche {n}",
  "nav.espace.titre": "→ ou espace",
  "planches.pos": "planche {n} / {total}",
  "planches.inserer.vide": "+ Planche vide",
  "planches.inserer.texte": "+ Planche de texte",
  "planches.apres": "Après la planche courante",
  "planches.hint":
    "Glissez une planche sur une autre pour la déplacer. Double-clic ouvre dans le Livre, ⌘L fige.",
  "tri.foot.vide":
    "Photos écartées par la curation ou retirées à la main. Un clic pour les détails, un double-clic repêche. Le tiroir du Livre les garde aussi à portée de glisser.",
  "tri.gardee.voir": "Voir la planche de la photo gardée",
  "tri.gardee.label": "gardée à sa place · voir la planche",
  "erreur.detail": "Détail technique",

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
  // -- les dix raisons d'écart (tri, revue, tiroir, bilan)
  "raison.retiree": "Retirées à la main",
  "raison.rejetee": "Rejetées dans votre logiciel photo",
  "raison.hors_budget": "Hors budget : bonnes photos, album plein",
  "raison.meme_moment": "Même moment, quasi la même photo",
  "raison.doublon": "Doublons de rafale ou de scène",
  "raison.jumeau": "Quasi identiques",
  "raison.panorama": "Panoramas : trop larges pour une page",
  "raison.definition": "Définition trop faible pour ce format",
  "raison.parasite": "Parasites : captures, images reçues",
  "raison.illisible": "Illisibles : fichiers endommagés ou tronqués",

  // -- le menu natif
  "menu.apropos": "À propos de Colophon",
  "menu.masquer": "Masquer Colophon",
  "menu.masquer.autres": "Masquer les autres",
  "menu.tout.afficher": "Tout afficher",
  "menu.quitter": "Quitter Colophon",
  "menu.preferences": "Préférences…",
  "menu.fichier": "Fichier",
  "menu.nouveau": "Nouveau…",
  "menu.ouvrir": "Ouvrir…",
  "menu.recents": "Albums récents",
  "menu.recents.vide": "Aucun album récent",
  "menu.enregistrer": "Enregistrer",
  "menu.exporter": "Exporter…",
  "menu.stockage": "Stockage…",
  "menu.fermer.album": "Fermer l’album",
  "menu.edition": "Édition",
  "menu.annuler": "Annuler",
  "menu.retablir": "Rétablir",
  "menu.couper": "Couper",
  "menu.copier": "Copier",
  "menu.coller": "Coller",
  "menu.tout.selectionner": "Tout sélectionner",
  "menu.affichage": "Affichage",
  "menu.livre": "Livre",
  "menu.tri": "Tri",
  "menu.planches": "Planches",
  "menu.envoi": "Envoi",
  "menu.couverture": "Couverture",
  "menu.fidele": "Aperçu fidèle",
  "menu.revue": "Passer en revue",
  "menu.reserve": "Photos en réserve",
  "menu.planche": "Planche",
  "menu.gabarit": "Gabarit…",
  "menu.dupliquer": "Dupliquer",
  "menu.figer": "Figer / libérer",
  "menu.rendre.auto": "Rendre à l’automatique…",
  "menu.inserer.vide": "Insérer une planche vide",
  "menu.inserer.texte": "Insérer une planche de texte",
  "menu.supprimer.planche": "Supprimer la planche",
  "menu.aide": "Aide",
  "menu.raccourcis": "Raccourcis clavier",
  "menu.signaler.bug": "Signaler un problème…",
  "menu.signaler.planche": "Signaler une planche ratée…",
  "menu.signaler.recadrage": "Signaler un recadrage raté…",

  // -- l'écran Envoi
  "envoi.dirty":
    "Des modifications ne sont pas enregistrées. Le prévol lit le fichier sur le disque : enregistrez (⌘S) avant de vous fier au verdict.",
  "envoi.controle": "Contrôle du fichier…",
  "envoi.ok": "Rien ne s’oppose à l’impression chez {imprimeur}.",
  "envoi.ok.deux": "{planches} planches, {pages} pages, intérieur et couverture en deux fichiers.",
  "envoi.ok.un":
    "{planches} planches, {pages} pages, un seul fichier de {fichier} pages, couverture comprise.",
  "envoi.ko.un": "Un défaut arrête l’envoi.",
  "envoi.ko": "{n} défauts arrêtent l’envoi.",
  "envoi.ko.sub":
    "Chaque ligne mène à sa planche. Corrigez, revenez, le contrôle se refait tout seul.",
  "envoi.defaut.album": "L’album",
  "envoi.defaut.planche": "Planche {n}",
  "envoi.imprimeurs": "Qui accepte un PDF comme celui-ci",
  "envoi.pdf.simple": "PDF simple",
  "envoi.rvb": "RVB",
  "envoi.cmjn": "CMJN FOGRA39",
  "envoi.deux.fichiers": "deux fichiers",
  "envoi.un.fichier": "un fichier",
  "envoi.dos.fournir": "dos à fournir",
  "envoi.dos.non": "dos non demandé",
  "envoi.provisoire": "fiche provisoire",
  "envoi.fiche.titre": "La fiche à donner à l’imprimeur",
  "envoi.fiche.format": "Format d’une page",
  "envoi.fiche.interieur": "Intérieur",
  "envoi.fiche.interieur.v": "{planches} planches, {pages} pages",
  "envoi.fiche.fond": "Fond perdu",
  "envoi.fiche.fond.v": "haut {haut}, bas {bas}, extérieur {ext}, dos {dos} mm",
  "envoi.fiche.zone": "Zone sûre",
  "envoi.fiche.zone.v": "{mm} mm depuis la coupe",
  "envoi.fiche.espace": "Espace couleur",
  "envoi.fiche.espace.cmjn": "CMJN",
  "envoi.fiche.conformite": "Conformité",
  "envoi.fiche.conformite.x4": "PDF/X-4 déclaré",
  "envoi.fiche.conformite.aucune": "aucune demandée",
  "envoi.fiche.livraison": "Livraison",
  "envoi.fiche.livraison.deux":
    "deux fichiers : l’intérieur et la couverture à plat",
  "envoi.fiche.livraison.un":
    "un seul fichier de {n} pages : couverture en première et en dernière page",
  "envoi.fiche.dos": "Dos",
  "envoi.fiche.dos.v": "{mm} mm pour {pages} pages à {g} g/m²",
  "envoi.fiche.resolution": "Résolution visée",
  "envoi.fiche.resolution.v": "{dpi} dpi",
  "envoi.reserves": "Ce que cette fiche attend encore",
  "envoi.garde.label": "Imprimer la page de garde",
  "envoi.garde.note":
    "La première page du livre, comme dans un livre imprimé : le titre, les dates du voyage, les villes traversées. Rien d’autre, et deux pages de plus.",
  "envoi.colophon.label": "Imprimer la page de colophon",
  "envoi.colophon.note":
    "La dernière page du livre, écrite par le logiciel : combien de photographies sur combien, quand, où, avec quels appareils. Deux pages de plus, et jamais un chemin, une coordonnée ni une légende.",
  "envoi.exporter": "Enregistrer le PDF d’impression",
  "envoi.exporter.rendu": "Rendu en cours…",
  "envoi.exporter.titre":
    "Rendu à 300 dpi, puis la couverture si l’imprimeur en veut une",
  "envoi.exporter.bloque": "Corrigez d’abord ce qui bloque",
  "envoi.porte":
    "Un imprimeur sans contrainte accepte souvent ce que {nom} refuse : essayez « Imprimeur local » ci-dessus pour voir ce qui resterait.",
  "envoi.verdict.titre": "Votre avis vaut une planche corrigée",
  "envoi.verdict.texte":
    "Deux questions, dix secondes : montreriez-vous cet album tel que le logiciel l’a composé, et quelles sont ses trois pires planches ? Chaque planche citée est examinée une par une.",
  "envoi.verdict.bouton":
    "Répondre sur GitHub (le formulaire pose ces deux questions)",

  // -- la table lumineuse
  "table.cellule.titre":
    "planche {n} · glisser pour déplacer, double-clic pour ouvrir",
  "table.editee": "Éditée à la main : survit à toute recomposition",
  "table.figee": "Figée : survit à toute recomposition. Cliquer pour libérer (⌘L)",
  "table.figer": "Figer cette planche face aux recompositions (⌘L)",
  "table.couverture": "Couverture",
  "table.couverture.titre": "Couverture · double-clic pour l’ouvrir",

  // -- la planche dans l'éditeur
  "planche.legende.deborde": "Cette légende dépasse la photo : raccourcissez-la",
  "planche.chapitre.placeholder": "Titre de chapitre…",
  "planche.chapitre.ghost": "titre de chapitre",
  "planche.proposition.titre":
    "Proposée depuis les photos : Tab la pose, tout autre geste l’ignore",
  "planche.chapitre.renommer": "Cliquer pour renommer le chapitre",
  "planche.legende": "Légende",
  "planche.legende.aucune": "aucune",
  "planche.legende.exif": "Date EXIF de la photo, proposée, jamais imposée",
  "planche.legende.proposer": "Proposer « {texte} »",
  "planche.texte.placeholder":
    "Votre texte, ligne à ligne.\nEntrée pour aller à la ligne.",
  "planche.texte.editer": "Cliquer pour éditer le texte",
  "planche.texte.ghost": "Page de texte : cliquer pour écrire.",

  // -- ce que le clavier tient de la planche : un nom par objet de la scène,
  //    bâti depuis le code de rôle et ses paramètres, jamais depuis une
  //    phrase venue du moteur.
  "scene.objets": "Objets de la planche",
  "scene.photo": "Photo {n} sur {total}, {fichier}",
  "scene.legende": "Légende de la photo {n} : {texte}",
  "scene.chapitre": "Titre de chapitre : {texte}",
  "scene.chapitre.vide": "Titre de chapitre, vide",
  "scene.texte": "Bloc de texte : {texte}",
  "planche.recadrer":
    "Glisser pour recadrer · molette pour zoomer · double-clic recentre · ⌥ affine",
  "planche.recadrer.pleine":
    "Cette photo remplit sa case exactement : il n’y a rien à faire glisser. Zoomez (molette ou +) pour vous donner du cadrage.",
  "planche.recadrer.pleine.status":
    "Photo à la taille exacte de sa case : zoomez (molette ou +) avant de recadrer",
  "planche.couverture.recadrer":
    "Glisser pour recadrer · molette pour zoomer · ⌥ affine",
  "planche.warn.ppi":
    "Cette photo imprimerait vers {ppi} ppi ici, sous le plancher de {plancher}. Une case plus petite, un zoom réduit ou une autre photo règlent le problème. L’export le signalera aussi.",
  "planche.warn.sombre.badge": "sombre",
  "planche.warn.sombre":
    "Photo très sombre : le papier la rendra plus sombre encore que l’écran. À garder en connaissance de cause, rien ne bloque.",
  "fidele.pdf.aria": "Aperçu fidèle, rendu depuis le PDF",
  "deborde.legende.horspage":
    "la légende de la case {i} tombe hors page sous une pleine page : retirez-la ou changez de gabarit",
  "deborde.legende.longue":
    "légende de la case {i} trop longue de {mm} mm : raccourcissez-la",
  "deborde.lignes": "{n} lignes de texte dépassent la page : coupez-les",
  "deborde.ligne.une": "1 ligne de texte dépasse la page : coupez-la",
  "deborde.garde":
    "la page de garde déborde : raccourcissez le titre de l’album",

  // -- le bilan de composition
  "bilan.titre": "« {titre} » est composé",
  "bilan.lues": "photos lues,",
  "bilan.gardees":
    "dans l’album, soit {pct} % du dossier : {planches} planches en {chapitres} chapitres.",
  "bilan.gardees.chapitre.un":
    "dans l’album, soit {pct} % du dossier : {planches} planches en 1 chapitre.",
  "bilan.choix.titre": "Trois livres, les mêmes photos",
  "bilan.demande.nom": "Comme demandé",
  "bilan.demande.about":
    "Le rythme et la longueur choisis à la création. Le point de départ.",
  "bilan.carte.chiffres": "{planches} planches, {photos} photos",
  "bilan.hint.ecartees":
    "Rien n’est supprimé : chaque photo écartée attend dans la vue Tri, avec sa raison, et un double-clic la repêche.",
  "bilan.hint.toutes": "Toutes les photos du dossier sont dans l’album.",
  "bilan.ouvrir": "Ouvrir l’album",
  "bilan.revue": "Passer les {n} écartées en revue",
  "bilan.garde":
    "Les deux autres restent sur le disque : elles se reprennent depuis cet écran tant que rien n’a été modifié à la main.",

  // -- l'éditeur de couverture
  "couverture.quatrieme":
    "Quatrième de couverture (optionnelle) : un mot, une dédicace, un été.",
  "couverture.dos.titre": "Dos {mm} mm",
  "couverture.dos.provisoire.titre": " (provisoire, en attente de l’imprimeur)",
  "couverture.choisir.photo": "Choisir la photo de couverture…",
  "couverture.titre.aria": "Titre de la couverture",
  "couverture.soustitre.placeholder": "sous-titre (optionnel)",
  "couverture.soustitre.aria": "Sous-titre",
  "couverture.changer.titre": "Choisir une autre photo de l’album",
  "couverture.changer": "Changer la photo",
  "couverture.note.dos": "Dos {mm} mm pour {pages} pages",
  "couverture.note.provisoire":
    ", valeur provisoire que la formule de l’imprimeur remplacera",
  "couverture.note.mince": " · trop mince pour porter un titre",
  "couverture.note.sans.dos": "{imprimeur} fabrique le dos : la feuille part sans lui.",
  "couverture.imprimeur": "L’imprimeur",
  "couverture.note.feuille": " · feuille {w} × {h} mm",
  "couverture.picker.titre": "Photo de couverture, parmi l’album",

  // -- les familles de gabarits (sélecteur, ligne de contexte)
  "gabarit.titre": "Gabarit de la planche",
  "gabarit.cycle": "Gabarit : {nom}",
  "gabarit.photos": "{n} photos",
  "gabarit.photos.une": "1 photo",
  "gabarit.full1": "Pleine page",
  "gabarit.solo": "Une photo",
  "gabarit.solo_paysage": "Une photo, paysage",
  "gabarit.solo_pano": "Une photo, panorama",
  "gabarit.solo_etroit": "Une photo, étroite",
  "gabarit.solo_carre": "Une photo, carrée",
  "gabarit.duo": "Deux photos",
  "gabarit.duo_portrait": "Deux portraits",
  "gabarit.duo_paysage": "Deux paysages",
  "gabarit.duo_etroit": "Deux photos, étroites",
  "gabarit.duo_pano": "Deux panoramas",
  "gabarit.trio": "Trois photos",
  "gabarit.trio_portrait": "Trois photos, portraits",
  "gabarit.quad": "Quatre photos",
  "gabarit.quad_portrait": "Quatre portraits",
  "gabarit.quad_etroit": "Quatre photos, étroites",
  "gabarit.quad_pano": "Quatre panoramas",
  "gabarit.six": "Six photos",
  "gabarit.octo": "Huit photos",
  "gabarit.texte": "Planche de texte",
  "gabarit.garde": "Page de garde",
  "gabarit.colophon": "Page de colophon",

  // -- la fiche des raccourcis (⌘/)
  "racc.titre": "Raccourcis clavier",
  "racc.naviguer": "Naviguer",
  "racc.editer": "Éditer la planche",
  "racc.recadrer": "Recadrer la photo sélectionnée",
  "racc.revue": "En revue (Tri)",
  "racc.album": "L’album",
  "racc.vues": "Livre, Tri, Planches, Envoi",
  "racc.planche.suiv": "Planche précédente, suivante",
  "racc.premiere": "Première, dernière planche",
  "racc.reserve": "Photos en réserve",
  "racc.fidele": "Aperçu fidèle : la page telle que le PDF la contient",
  "racc.passer.revue": "Passer en revue",
  "racc.dupliquer": "Dupliquer la planche",
  "racc.figer": "Figer ou libérer la planche",
  "racc.supprimer": "Supprimer la planche",
  "racc.envoyer.photo": "Envoyer la photo sur la planche voisine",
  "racc.retirer.photo": "Retirer la photo sélectionnée",
  "racc.tab.legende": "Poser la légende proposée",
  "racc.gabarit": "Gabarit suivant, précédent",
  "racc.deplacer.cadrage": "Déplacer le cadrage",
  "racc.zoomer": "Zoomer, dézoomer",
  "racc.remplissage": "Revenir au remplissage exact",
  "racc.recentrer": "Recentrer sur le visage détecté",
  "racc.parcourir": "Parcourir les écartées",
  "racc.repecher": "Repêcher",
  "racc.ecart": "Écart confirmé, photo suivante",
  "racc.sortir": "Sortir de la revue",
  "racc.enregistrer": "Enregistrer",
  "racc.annuler": "Annuler, rétablir",
  "racc.exporter": "Exporter (ouvre Envoi)",
  "racc.ouvrir": "Ouvrir, nouveau",
  "racc.k.espace": "← → · espace",
  "racc.k.debut": "Début / Fin",
  "racc.k.entree": "Entrée (Tri)",
  "racc.k.suppr.planches": "⌫ (Planches)",
  "racc.k.suppr.livre": "⌫ (Livre)",
  "racc.k.glisser": "glisser · ⌥ affine",
  "racc.k.molette": "molette · + −",
  "racc.k.doubleclic": "double-clic",
  "racc.k.echap": "Échap",

  // -- le tiroir de photos
  "tiroir.reserve": "Photos en réserve",
  "tiroir.non.placees": "Non placées",
  "tiroir.ecartees": "Écartées",
  "tiroir.hint": "glissez une photo sur une case du livre pour l’y placer",
  "tiroir.vide.non.placees":
    "Aucune photo en attente : tout ce qui mérite l’album y est.",
  "tiroir.vide.ecartees": "Rien d’écarté par la curation.",
  "tiroir.gardee": " (gardée : {gardee})",

  // -- la vue Tri et la revue
  "tri.vide": "Rien à trier : toutes les photos du dossier sont dans l’album.",
  "tri.lede":
    "{n} photos hors de l’album, chacune avec sa raison. Un double-clic repêche.",
  "tri.lede.une":
    "1 photo hors de l’album, avec sa raison. Un double-clic repêche.",
  "tri.revue": "Passer en revue",
  "tri.gardee": "{nom}, gardée : {gardee}",
  "revue.gardee": ", gardée à sa place : {gardee}",
  "revue.repecher": "Repêcher",
  "revue.confirme": "Écart confirmé",
  "revue.parcourir": "parcourir",
  "revue.sortir": "Sortir",

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

  "commun.fermer": "Close (Esc)",

  "stockage.titre": "Storage",
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
  "apropos.attribution":
    "Place names: GeoNames data (https://www.geonames.org), under the Creative Commons Attribution 4.0 licence.",
  "apropos.icc.licence":
    "International Color Consortium, unrestricted redistribution",

  "signaler.titre.bug": "Report a problem",
  "signaler.titre.planche": "Report a bad spread",
  "signaler.titre.recadrage": "Report a bad crop",
  "signaler.intro":
    "The report below is built on this machine. Read it: it is everything that leaves, nothing else. Numbers and file names, never a path, a GPS coordinate or a caption.",
  "signaler.construction":
    "Building the report… The audit rereads every photo, a few seconds on a large album.",
  "signaler.piece":
    "I will attach a picture of the spread myself on the GitHub page. Nothing is uploaded from here.",
  "signaler.ouvrir": "Open the pre-filled GitHub issue",
  "signaler.copier": "Copy the report",
  "signaler.copie": "Report copied",
  "signaler.note.ouverte":
    "The issue opens in your browser, the report already in place: reread, complete, publish.",
  "signaler.note.copie":
    "No network, or no GitHub account: copy the report, it pastes as is into an issue or an email, later.",

  "rapport.album": "Album: {w} × {h} mm, {n} spreads",
  "rapport.planche":
    "Spread {n} of {total}, template {gabarit}, {photos}{edition}{figee}:",
  "rapport.photos": "{n} photos",
  "rapport.photos.une": "1 photo",
  "rapport.editee": ", edited by hand",
  "rapport.figee": ", locked",
  "rapport.case": "  cell {i}: {geo}, photo {nom}",
  "rapport.case.hors.gabarit": "cell outside the template",
  "rapport.hors.gabarit": "outside the template",
  "rapport.paysage": "landscape",
  "rapport.portrait": "portrait",
  "rapport.carree": "square",
  "rapport.case.signalee":
    "Reported cell: cell {i}, {geo}, photo {nom}, focal point {fx} ; {fy}, zoom {zoom}",
  "rapport.audit.indisponible":
    "Audit: unavailable (no album open, or the audit failed)",
  "rapport.audit.rouges": "{n} counters over their threshold",
  "rapport.audit.rouge.un": "1 counter over its threshold",
  "rapport.audit.verts": "every counter under its threshold",
  "rapport.audit": "Audit ({n} spreads): {verdict}",
  "rapport.audit.note": "note: {note}",
  "rapport.image":
    "Picture of the spread: attached by hand in GitHub, on purpose.",
  "rapport.log":
    "Log extract (paths already reduced to file names):",

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
  "prefs.rendu": "How spreads are drawn",
  "prefs.rendu.dom": "Elements",
  "prefs.rendu.canvas": "Canvas",
  "prefs.rendu.note":
    "Both draw the same spread, from the same objects; they differ only in how it reaches the screen. The canvas is under evaluation: if dragging a crop feels heavy on your machine, try it. The PDF never changes, whichever this is set to.",

  "setup.nouvel": "New album",
  "setup.changer.dossier": "Change folder",
  "setup.titre": "title",
  "setup.planches": "spreads",
  "setup.pages": "that is {n} pages",
  "setup.format": "page format",
  "setup.rythme": "pace",
  "setup.rythme.hint":
    "The pace replays at every recomposition; each spread stays editable one by one.",
  "setup.composer": "Compose the album",
  "setup.annuler": "Cancel",
  "setup.cm.ouvert": "{cm} cm open",
  "setup.la.page": "the page",
  "format.carre-21": "Square 21 × 21",
  "format.carre-30": "Square 30 × 30",
  "format.portrait-a4": "Portrait A4",
  "format.paysage-a4": "Landscape A4",
  "format.paysage-28x21": "Landscape 28 × 21",
  "format.portrait-20x25": "Portrait 20 × 25",
  "compo.recomposition": "Recomposing “{titre}”",
  "compo.composition": "Composing “{titre}”",
  "compo.album.defaut": "the album",
  "compo.hint.recomp":
    "Spreads edited by hand or locked are kept exactly as they are.",
  "compo.hint.compo":
    "The photo analysis only runs once: recomposing this folder will be much faster.",
  "compo.arreter.titre": "Stops the composition; nothing is written",
  "compo.details": "Technical details",
  "compo.lecture": "reading the folder…",
  "compo.annulee": "Composition cancelled",
  "compo.vide":
    "This folder yielded no usable photo, nothing was created. Choose another folder, or reopen this one after adding photos to it.",
  "erreur.compo": "The composition failed.",
  "recomp.confirme":
    "Recompose the album? Spreads edited by hand or locked are kept exactly as they are, the others are recomposed. The undo history starts over.",
  "recomp.ok": "Album recomposed, edited spreads kept",
  "recomp.annulee": "Recomposition cancelled",
  "erreur.recomp": "The recomposition failed.",
  "fermer.confirme": "Some changes are not saved. Close anyway?",
  "stage.lecture": "reading the folder",
  "stage.scan": "folder inventory",
  "stage.analyse.n": "analysing the photos, {i} of {n}",
  "stage.analyse": "analysing the photos",
  "stage.parasites": "setting the strays aside",
  "stage.dedup": "deduplicating the bursts",
  "stage.eclaircissage": "thinning the duplicates",
  "stage.chapitres": "cutting into chapters",
  "stage.layout": "laying out the spreads",
  "stage.pinned": "edited spreads put back",
  "stage.curation": "curation journal",
  "stage.pdf": "rendering the PDF",

  "export.rendu": "Rendering the print PDF…",
  "export.progress": "Rendering at 300 dpi: {done}/{total} photos…",
  "export.enregistrement.annule": "Save cancelled",
  "export.fichiers": "{n} files saved: {liste}",
  "export.pdf": "PDF saved: {nom}",
  "export.annule": "Export cancelled, no file written",
  "export.annuler": "Cancel the export",
  "erreur.export": "Rendering the PDF failed.",

  "repeche.place":
    "No room around spread {n}: free a cell or change a template",
  "repeche.ok": "Rescued onto spread {n}",
  "place.doublon": "Already on this spread: the same photo twice would be a duplicate",
  "place.remplacee": "Photo placed · the old one goes back on hand",
  "place.ok": "Photo placed",
  "planche.dupliquee": "Spread {n} duplicated",
  "planche.liberee": "Spread released",
  "planche.figee.status": "Spread locked: it will survive any recomposition",
  "auto.confirme":
    "Give spread {n} back to the machine?\n\nIt takes back the composition proposed at the start. The crop, the captions and the photos changed by hand on this spread are lost, and the padlock falls.\n\n⌘Z goes back.",
  "planche.vide.inseree": "Empty spread inserted: a breath",
  "planche.texte.inseree": "Text spread inserted: double-click to open it and write",
  "planche.supprimee": "Spread {n} deleted (⌘Z brings it back)",
  "planche.deplacee": "Spread moved to position {n}",
  "signal.planche.dabord":
    "Open the spread to report first (Book or Spreads view)",
  "signal.case.dabord": "Select the badly cropped cell first (Book view)",
  "revue.terminee": "Review finished, every discard was seen",
  "move.pleine": "Spread {n} is full: no template takes one more photo",
  "move.texte": "Spread {n}: a text page, a photo would erase it",
  "move.refuse": "Refused: it would take sacrificing another photo of this spread",
  "move.ok": "Photo sent to spread {n}",
  "zoom.remis": "Zoom back to the exact fill",
  "legende.posee": "Caption set: “{texte}” (⌘Z removes it)",

  "bar.recomposer.titre":
    "Recomposes the album; edited or locked spreads are kept",
  "contexte.couverture":
    "The cover: title and subtitle in place, drag the photo to recrop it. The photo drawer returns on the spreads.",
  "contexte.recadrage":
    "Crop: drag moves, wheel zooms, ⌥ refines, ⌫ removes the photo",
  "nav.precedente": "Previous spread",
  "nav.suivante": "Next spread",
  "nav.aller": "Go to a spread",
  "nav.planche": "spread {n}",
  "nav.espace.titre": "→ or space",
  "planches.pos": "spread {n} / {total}",
  "planches.inserer.vide": "+ Empty spread",
  "planches.inserer.texte": "+ Text spread",
  "planches.apres": "After the current spread",
  "planches.hint":
    "Drag a spread onto another to move it. Double-click opens it in the Book, ⌘L locks.",
  "tri.foot.vide":
    "Photos set aside by the curation or removed by hand. One click for the details, a double-click rescues. The Book’s drawer keeps them within dragging reach too.",
  "tri.gardee.voir": "See the spread of the kept photo",
  "tri.gardee.label": "kept in its place · see the spread",
  "erreur.detail": "Technical detail",

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
  "menu.apropos": "About Colophon",
  "menu.masquer": "Hide Colophon",
  "menu.masquer.autres": "Hide Others",
  "menu.tout.afficher": "Show All",
  "menu.quitter": "Quit Colophon",
  "menu.preferences": "Preferences…",
  "menu.fichier": "File",
  "menu.nouveau": "New…",
  "menu.ouvrir": "Open…",
  "menu.recents": "Recent albums",
  "menu.recents.vide": "No recent album",
  "menu.enregistrer": "Save",
  "menu.exporter": "Export…",
  "menu.stockage": "Storage…",
  "menu.fermer.album": "Close the album",
  "menu.edition": "Edit",
  "menu.annuler": "Undo",
  "menu.retablir": "Redo",
  "menu.couper": "Cut",
  "menu.copier": "Copy",
  "menu.coller": "Paste",
  "menu.tout.selectionner": "Select All",
  "menu.affichage": "View",
  "menu.livre": "Book",
  "menu.tri": "Sort",
  "menu.planches": "Spreads",
  "menu.envoi": "Send",
  "menu.couverture": "Cover",
  "menu.fidele": "Faithful preview",
  "menu.revue": "Review the discards",
  "menu.reserve": "Photos on hand",
  "menu.planche": "Spread",
  "menu.gabarit": "Template…",
  "menu.dupliquer": "Duplicate",
  "menu.figer": "Lock / release",
  "menu.rendre.auto": "Give back to the machine…",
  "menu.inserer.vide": "Insert an empty spread",
  "menu.inserer.texte": "Insert a text spread",
  "menu.supprimer.planche": "Delete the spread",
  "menu.aide": "Help",
  "menu.raccourcis": "Keyboard shortcuts",
  "menu.signaler.bug": "Report a problem…",
  "menu.signaler.planche": "Report a bad spread…",
  "menu.signaler.recadrage": "Report a bad crop…",

  "envoi.dirty":
    "Some changes are not saved. The preflight reads the file on disk: save (⌘S) before trusting the verdict.",
  "envoi.controle": "Checking the file…",
  "envoi.ok": "Nothing stands against printing at {imprimeur}.",
  "envoi.ok.deux": "{planches} spreads, {pages} pages, interior and cover as two files.",
  "envoi.ok.un":
    "{planches} spreads, {pages} pages, a single file of {fichier} pages, cover included.",
  "envoi.ko.un": "One defect stops the send.",
  "envoi.ko": "{n} defects stop the send.",
  "envoi.ko.sub":
    "Each line leads to its spread. Fix, come back, the check reruns on its own.",
  "envoi.defaut.album": "The album",
  "envoi.defaut.planche": "Spread {n}",
  "envoi.imprimeurs": "Who accepts a PDF like this one",
  "envoi.pdf.simple": "plain PDF",
  "envoi.rvb": "RGB",
  "envoi.cmjn": "CMYK FOGRA39",
  "envoi.deux.fichiers": "two files",
  "envoi.un.fichier": "one file",
  "envoi.dos.fournir": "spine to supply",
  "envoi.dos.non": "spine not asked for",
  "envoi.provisoire": "provisional sheet",
  "envoi.fiche.titre": "The sheet to hand the printer",
  "envoi.fiche.format": "Page size",
  "envoi.fiche.interieur": "Interior",
  "envoi.fiche.interieur.v": "{planches} spreads, {pages} pages",
  "envoi.fiche.fond": "Bleed",
  "envoi.fiche.fond.v": "top {haut}, bottom {bas}, outer {ext}, spine {dos} mm",
  "envoi.fiche.zone": "Safe zone",
  "envoi.fiche.zone.v": "{mm} mm from the trim",
  "envoi.fiche.espace": "Colour space",
  "envoi.fiche.espace.cmjn": "CMYK",
  "envoi.fiche.conformite": "Conformance",
  "envoi.fiche.conformite.x4": "PDF/X-4 declared",
  "envoi.fiche.conformite.aucune": "none asked",
  "envoi.fiche.livraison": "Delivery",
  "envoi.fiche.livraison.deux": "two files: the interior and the flat cover",
  "envoi.fiche.livraison.un":
    "a single file of {n} pages: cover as first and last page",
  "envoi.fiche.dos": "Spine",
  "envoi.fiche.dos.v": "{mm} mm for {pages} pages at {g} g/m²",
  "envoi.fiche.resolution": "Target resolution",
  "envoi.fiche.resolution.v": "{dpi} dpi",
  "envoi.reserves": "What this sheet still waits on",
  "envoi.garde.label": "Print the half-title",
  "envoi.garde.note":
    "The first page of the book, the way printed books open: the title, the dates of the trip, the towns crossed. Nothing else, and two more pages.",
  "envoi.colophon.label": "Print the colophon page",
  "envoi.colophon.note":
    "The last page of the book, written by the software: how many photographs out of how many, when, where, with which cameras. Two more pages, and never a path, a coordinate or a caption.",
  "envoi.exporter": "Save the print PDF",
  "envoi.exporter.rendu": "Rendering…",
  "envoi.exporter.titre":
    "Rendered at 300 dpi, then the cover if the printer wants one",
  "envoi.exporter.bloque": "Fix what blocks first",
  "envoi.porte":
    "A printer without constraints often accepts what {nom} refuses: try “Local printer” above to see what would remain.",
  "envoi.verdict.titre": "Your verdict is worth a corrected spread",
  "envoi.verdict.texte":
    "Two questions, ten seconds: would you show this album exactly as the software composed it, and which are its three worst spreads? Every cited spread is examined one by one.",
  "envoi.verdict.bouton": "Answer on GitHub (the form asks these two questions)",

  "table.cellule.titre":
    "spread {n} · drag to move, double-click to open",
  "table.editee": "Edited by hand: survives any recomposition",
  "table.figee": "Locked: survives any recomposition. Click to release (⌘L)",
  "table.figer": "Lock this spread against recompositions (⌘L)",
  "table.couverture": "Cover",
  "table.couverture.titre": "Cover · double-click to open it",

  "planche.legende.deborde": "This caption overflows the photo: shorten it",
  "planche.chapitre.placeholder": "Chapter title…",
  "planche.chapitre.ghost": "chapter title",
  "planche.proposition.titre":
    "Proposed from the photos: Tab takes it, any other gesture ignores it",
  "planche.chapitre.renommer": "Click to rename the chapter",
  "planche.legende": "Caption",
  "planche.legende.aucune": "none",
  "planche.legende.exif": "EXIF date of the photo, proposed, never imposed",
  "planche.legende.proposer": "Propose “{texte}”",
  "planche.texte.placeholder": "Your text, line by line.\nEnter for a new line.",
  "planche.texte.editer": "Click to edit the text",
  "planche.texte.ghost": "Text page: click to write.",

  // -- what the keyboard holds a spread by
  "scene.objets": "Objects on this spread",
  "scene.photo": "Photo {n} of {total}, {fichier}",
  "scene.legende": "Caption of photo {n}: {texte}",
  "scene.chapitre": "Chapter title: {texte}",
  "scene.chapitre.vide": "Chapter title, empty",
  "scene.texte": "Text block: {texte}",
  "planche.recadrer":
    "Drag to crop · wheel to zoom · double-click recentres · ⌥ refines",
  "planche.recadrer.pleine":
    "This photo fills its cell exactly: there is nothing to slide. Zoom in (wheel or +) to give yourself some framing.",
  "planche.recadrer.pleine.status":
    "Photo exactly the size of its cell: zoom in (wheel or +) before cropping",
  "planche.couverture.recadrer": "Drag to crop · wheel to zoom · ⌥ refines",
  "planche.warn.ppi":
    "This photo would print near {ppi} ppi here, under the {plancher} floor. A smaller cell, less zoom or another photo fixes it. The export will flag it too.",
  "planche.warn.sombre.badge": "dark",
  "planche.warn.sombre":
    "Very dark photo: paper will print it darker still than the screen. Keep it knowingly, nothing blocks.",
  "fidele.pdf.aria": "Faithful preview, rendered from the PDF",
  "deborde.legende.horspage":
    "the caption of cell {i} falls off the page under a full bleed: remove it or change the template",
  "deborde.legende.longue":
    "caption of cell {i} too long by {mm} mm: shorten it",
  "deborde.lignes": "{n} lines of text overflow the page: cut them",
  "deborde.ligne.une": "1 line of text overflows the page: cut it",
  "deborde.garde": "the half-title overflows: shorten the album title",

  "bilan.titre": "“{titre}” is composed",
  "bilan.lues": "photos read,",
  "bilan.gardees":
    "in the album, {pct} % of the folder: {planches} spreads in {chapitres} chapters.",
  "bilan.gardees.chapitre.un":
    "in the album, {pct} % of the folder: {planches} spreads in 1 chapter.",
  "bilan.choix.titre": "Three books, the same photos",
  "bilan.demande.nom": "As asked",
  "bilan.demande.about":
    "The pace and the length chosen at creation. The starting point.",
  "bilan.carte.chiffres": "{planches} spreads, {photos} photos",
  "bilan.hint.ecartees":
    "Nothing is deleted: every photo set aside waits in the Sort view, with its reason, and a double-click rescues it.",
  "bilan.hint.toutes": "Every photo in the folder is in the album.",
  "bilan.ouvrir": "Open the album",
  "bilan.revue": "Review the {n} discards",
  "bilan.garde":
    "The two others stay on disk: they can be taken up from this screen as long as nothing was edited by hand.",

  "couverture.quatrieme":
    "Back cover (optional): a word, a dedication, a summer.",
  "couverture.dos.titre": "Spine {mm} mm",
  "couverture.dos.provisoire.titre": " (provisional, waiting on the printer)",
  "couverture.choisir.photo": "Choose the cover photo…",
  "couverture.titre.aria": "Cover title",
  "couverture.soustitre.placeholder": "subtitle (optional)",
  "couverture.soustitre.aria": "Subtitle",
  "couverture.changer.titre": "Choose another photo from the album",
  "couverture.changer": "Change the photo",
  "couverture.note.dos": "Spine {mm} mm for {pages} pages",
  "couverture.note.provisoire":
    ", provisional value the printer’s formula will replace",
  "couverture.note.mince": " · too thin to carry a title",
  "couverture.note.sans.dos": "{imprimeur} makes the spine: the sheet leaves without it.",
  "couverture.imprimeur": "The printer",
  "couverture.note.feuille": " · sheet {w} × {h} mm",
  "couverture.picker.titre": "Cover photo, from the album",

  "gabarit.titre": "Template of the spread",
  "gabarit.cycle": "Template: {nom}",
  "gabarit.photos": "{n} photos",
  "gabarit.photos.une": "1 photo",
  "gabarit.full1": "Full page",
  "gabarit.solo": "One photo",
  "gabarit.solo_paysage": "One photo, landscape",
  "gabarit.solo_pano": "One photo, panorama",
  "gabarit.solo_etroit": "One photo, narrow",
  "gabarit.solo_carre": "One photo, square",
  "gabarit.duo": "Two photos",
  "gabarit.duo_portrait": "Two portraits",
  "gabarit.duo_paysage": "Two landscapes",
  "gabarit.duo_etroit": "Two photos, narrow",
  "gabarit.duo_pano": "Two panoramas",
  "gabarit.trio": "Three photos",
  "gabarit.trio_portrait": "Three photos, portraits",
  "gabarit.quad": "Four photos",
  "gabarit.quad_portrait": "Four portraits",
  "gabarit.quad_etroit": "Four photos, narrow",
  "gabarit.quad_pano": "Four panoramas",
  "gabarit.six": "Six photos",
  "gabarit.octo": "Eight photos",
  "gabarit.texte": "Text spread",
  "gabarit.garde": "Half-title",
  "gabarit.colophon": "Colophon page",

  "racc.titre": "Keyboard shortcuts",
  "racc.naviguer": "Navigate",
  "racc.editer": "Edit the spread",
  "racc.recadrer": "Crop the selected photo",
  "racc.revue": "In review (Sort)",
  "racc.album": "The album",
  "racc.vues": "Book, Sort, Spreads, Send",
  "racc.planche.suiv": "Previous, next spread",
  "racc.premiere": "First, last spread",
  "racc.reserve": "Photos on hand",
  "racc.fidele": "Faithful preview: the page as the PDF holds it",
  "racc.passer.revue": "Review the discards",
  "racc.dupliquer": "Duplicate the spread",
  "racc.figer": "Lock or release the spread",
  "racc.supprimer": "Delete the spread",
  "racc.envoyer.photo": "Send the photo to the next spread",
  "racc.retirer.photo": "Remove the selected photo",
  "racc.tab.legende": "Take the proposed caption",
  "racc.gabarit": "Next, previous template",
  "racc.deplacer.cadrage": "Move the crop",
  "racc.zoomer": "Zoom in, out",
  "racc.remplissage": "Back to the exact fill",
  "racc.recentrer": "Recentre on the detected face",
  "racc.parcourir": "Browse the discards",
  "racc.repecher": "Rescue",
  "racc.ecart": "Discard confirmed, next photo",
  "racc.sortir": "Leave the review",
  "racc.enregistrer": "Save",
  "racc.annuler": "Undo, redo",
  "racc.exporter": "Export (opens Send)",
  "racc.ouvrir": "Open, new",
  "racc.k.espace": "← → · space",
  "racc.k.debut": "Home / End",
  "racc.k.entree": "Enter (Sort)",
  "racc.k.suppr.planches": "⌫ (Spreads)",
  "racc.k.suppr.livre": "⌫ (Book)",
  "racc.k.glisser": "drag · ⌥ refines",
  "racc.k.molette": "wheel · + −",
  "racc.k.doubleclic": "double-click",
  "racc.k.echap": "Esc",

  "tiroir.reserve": "Photos on hand",
  "tiroir.non.placees": "Not placed",
  "tiroir.ecartees": "Set aside",
  "tiroir.hint": "drag a photo onto a cell of the book to place it there",
  "tiroir.vide.non.placees":
    "No photo waiting: everything that deserves the album is in it.",
  "tiroir.vide.ecartees": "Nothing set aside by the curation.",
  "tiroir.gardee": " (kept: {gardee})",

  "raison.retiree": "Removed by hand",
  "raison.rejetee": "Rejected in your photo software",
  "raison.hors_budget": "Over budget: good photos, full album",
  "raison.meme_moment": "Same moment, almost the same photo",
  "raison.doublon": "Burst or scene duplicates",
  "raison.jumeau": "Near identical",
  "raison.panorama": "Panoramas: too wide for a page",
  "raison.definition": "Resolution too low for this format",
  "raison.parasite": "Strays: screenshots, received images",
  "raison.illisible": "Unreadable: damaged or truncated files",

  "tri.vide": "Nothing to sort: every photo in the folder is in the album.",
  "tri.lede":
    "{n} photos out of the album, each with its reason. Double-click rescues.",
  "tri.lede.une":
    "1 photo out of the album, with its reason. Double-click rescues.",
  "tri.revue": "Review them",
  "tri.gardee": "{nom}, kept: {gardee}",
  "revue.gardee": ", kept in its place: {gardee}",
  "revue.repecher": "Rescue",
  "revue.confirme": "Discard confirmed",
  "revue.parcourir": "browse",
  "revue.sortir": "Leave",

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
