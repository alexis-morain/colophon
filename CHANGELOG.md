# Journal des versions

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et
la numérotation le [versionnage sémantique](https://semver.org/lang/fr/).

Une release par semaine le premier mois qui suit le lancement. Les binaires
macOS et Windows, leurs empreintes SHA-256 et ces notes sont publiés ensemble
sur la page des releases.

## [Non publié]

### Ajouté

- **Copier, couper, coller, dupliquer et pousser un bloc de texte ou un
  ornement au clavier.** Choisissez l'objet sur la planche : ⌘C le copie,
  ⌘X le coupe, ⌘V le colle sur la planche affichée, et le menu Édition fait
  la même chose. Collé sur sa propre planche, il se pose 4 mm plus bas et à
  droite pour qu'on voie la copie ; sur une autre, à la même place, hors des
  photos quand la page a de la place. ⌘D duplique l'objet choisi, et la
  planche quand rien n'est choisi. Les flèches le poussent d'un millimètre,
  de cinq avec ⇧, et il bute au pli comme à la souris. ⌫ le retire. Chaque
  geste s'annule d'un ⌘Z. Un champ de saisie garde son copier-coller à lui.

- **Choisir la police du livre.** Dans *Format*, à côté du format de page :
  dix familles, une par voix — une linéale neutre, une humaniste, une
  géométrique, un romain classique, un romain de texte, une didone, une
  égyptienne, une machine à écrire, un romain élégant, une chasse fixe —,
  chacune **écrite dans sa propre police**, avec une ligne de spécimen : on
  voit la police avant de la choisir, et ce qui est dessiné à l'écran est ce
  que le PDF embarquera. Toutes les polices de la machine restent à un clic,
  groupées par famille, avec un champ pour filtrer. Une police pour tout le
  livre — légendes, titres de
  chapitre, page de garde, colophon, couverture et dos. Rien n'est
  recomposé : les planches, les photos et les recadrages ne bougent pas,
  seules les coupures de ligne suivent. ⌘Z annule le choix, ⌘S l'enregistre.
  La police retenue est **copiée dans le dossier de l'album**, donc l'album
  s'ouvre et s'imprime à l'identique sur une machine qui ne l'a pas.
  Les polices que leur licence interdit d'incorporer, ou qu'un PDF ne peut
  pas porter, restent dans la liste, grisées, avec la raison : mieux vaut
  lire pourquoi qu'aller la chercher. Et si le fichier de la police disparaît
  du dossier, l'album sort dans celle de Colophon et l'écran le dit — jamais
  un livre imprimé dans une police que personne n'a choisie.

- **Page de garde.** La première page du livre, comme dans un livre imprimé :

  le titre de l'album, les dates du voyage, les villes traversées. Trois
  lignes, rien d'autre. Activée par défaut, décochable dans Envoi à côté de
  la page de colophon. Le titre suit le renommage de l'album ; les dates et
  les villes sortent de ce que la composition a mesuré, jamais d'une phrase
  écrite à votre place. Le titre imprimé est celui du livre : celui de la
  couverture quand vous lui en avez donné un, celui de l'album sinon. Un
  titre trop long pour la page rétrécit plutôt que de déborder, et rien n'est
  jamais coupé.

- **Trente-deux ornements de plus.** Le sélecteur en montre trente-cinq,
  rangés en trois groupes : quatorze fleurons, treize filets, huit
  séparateurs. Tous viennent de Wikimedia Commons, tous sont dans le domaine
  public ou sous CC0, et leur provenance est listée dans
  `assets/ornements/LICENCES.md`. Pour en proposer un autre,
  `scripts/ornement-normaliser.py` ramène un SVG dans ce que le PDF sait
  tracer, et refuse en le nommant ce qu'il ne sait pas traduire.

### Modifié

- **Les réglages d'un bloc de texte s'ouvrent sous le bloc.** Corps,
  interligne, angle et alignement vivaient dans la barre sous la planche, où
  ils s'empilaient faute de place dès que la fenêtre rétrécissait. Ils
  s'ouvrent désormais juste sous le bloc choisi (au-dessus quand le bas de
  la fenêtre est proche), sur deux rangées. L'alignement se
  choisit par trois boutons à icône au lieu d'un menu. Un ornement n'y montre
  que son angle. Échap abandonne une valeur en cours de saisie, et chaque
  réglage reste un seul pas d'annulation.

- **Le choix Éléments / Canvas quitte les Préférences.** Il ne changeait
  rien au livre ni au PDF, et ne servait qu'à mesurer deux façons de dessiner
  la planche à l'écran. Les Préférences ne gardent que la langue, les mises à
  jour et l'apparence.

- **La vérification des mises à jour se dit, et se coupe.** Elle existait
  depuis la 0.9.0 et partait au lancement sans condition, pendant que
  `SECURITY.md` promettait « no network call at runtime » et le README
  « Fully offline » : la promesse était fausse, pas le code. Les deux textes
  nomment désormais ce qui sort — une requête vers la page des versions, dont
  GitHub voit l'adresse IP et le nom du système, rien des photographies, rien
  de l'album, aucun identifiant fabriqué par Colophon — et *Préférences* (⌘,)
  porte l'interrupteur. Le réglage agit dans la seconde : coupé, le bandeau
  disparaît et plus rien ne sort ; rallumé, la question est posée tout de
  suite. Il reste allumé par défaut, parce que pour une application
  distribuée hors de toute boutique, c'est le seul chemin par lequel un
  correctif arrive. Rien ne s'est jamais installé sans un clic, et rien ne
  change de ce côté.

- **Le sélecteur de gabarits montre des dispositions, pas des gabarits.** Une
  planche de quatre photos en proposait jusqu'à 171, et la moitié de ce
  nombre était le même dessin deux fois : une bande de légende de huit
  millimètres, invisible à la taille d'une vignette, ou une forme de cellule
  que la vignette montre déjà. Il en montre au plus vingt-trois, une par
  disposition, groupées par nombre de photos et **nommées** — « Deux en
  colonne par page », « Une pleine page, trois côte à côte » — là où les
  gabarits ajoutés au catalogue s'affichaient encore sous leur nom de
  fichier, `g_1x2f_1x2f`. Le gabarit réellement posé est la variante que ces
  photos-là cadrent le mieux, jugée par le moteur et non choisie à la main.
  G et ⇧G parcourent la même liste.

- **Envoi ne laisse plus deux cents pixels de vide** entre le verdict et les
  défauts qu'il annonce : les deux colonnes ne partagent plus leurs lignes.
  Et plus rien dans l'interface ne se lit sous douze pixels : les pastilles
  sur les photos, les noms de groupes des sélecteurs, les licences d'À propos
  et la mention « fiche provisoire » montent d'un pixel.

### Corrigé

- **Les planches se réordonnent à la souris.** Glisser une planche sur une
  autre dans *Planches* ne faisait rien dans l'application installée. Le
  glisser est réécrit : la planche suit la souris, la planche visée se marque
  d'un pointillé, et le déplacement se fait au relâchement. Un simple clic
  choisit toujours la planche, Échap pendant le glisser annule sans rien
  changer, et ⌘Z ramène la planche à sa place. ⌥ flèches marche comme avant.

- **La légende d'une photo ne déborde plus.** Son champ prend la largeur de
  la photo, entre 240 et 420 pixels, au lieu d'une largeur fixe qui pouvait
  mordre sur la voisine ; une date proposée trop longue se coupe d'une
  ellipse, entière au survol. La phrase d'aide sous la planche ne recouvre
  plus rien quand la fenêtre est étroite : elle se coupe elle aussi, et se lit
  en entier au survol. Les boutons de la barre ne passent plus sur plusieurs
  lignes.

- **Le cache de vignettes s'élague.** Une photo retouchée, renommée ou
  retirée du dossier laissait son ancienne vignette pour toujours, 227 Ko
  chacune, sans limite d'âge ni de taille. À chaque composition, ce que
  l'index ne nomme plus est retiré, et le journal dit combien.

- **Quitter et fermer la fenêtre demandent, comme tout le reste.** ⌘Q et la
  pastille rouge jetaient un album modifié sans un mot, alors que *Fermer*
  et *Recomposer* demandaient tous les deux : le menu *Quitter* était celui
  du système, qui passe par une terminaison native et ne laisse aucun endroit
  où poser la question. Il est à nous désormais, ⌘Q compris, et la fermeture
  de fenêtre passe par la même question. Le garde du navigateur de
  développement, qui ne faisait rien faute de `returnValue`, en profite.
- **Ouvrir un autre album ne jette plus le travail en cours.** ⌘O et la liste
  des albums récents, dans le menu comme sur l'écran d'accueil, remplaçaient
  l'album ouvert sans rien demander, pile d'annulation comprise. Ils
  demandent, avec la même phrase que fermer.
- **Enregistrer ne supprime plus les sauvegardes faites à la main.** Le projet
  annonce `album.json` réparable dans un éditeur de texte, donc il invite
  exactement le geste qu'il punissait : copier l'album avant une édition
  risquée. `album.sauvegarde.json` mourait au ⌘S suivant, le ménage des
  propositions non choisies reconnaissant tout fichier `album.quelquechose.json`
  au lieu des seuls noms qu'il avait lui-même écrits. Il ne supprime plus que
  ceux-là. Une proposition périmée qui traînerait coûte un fichier ; une
  sauvegarde effacée coûtait une soirée.
- **Un titre imprime les caractères qu'il porte, et plus des points
  d'interrogation.** L'éditeur affichait « Zażółć », le PDF imprimait
  « Za?ó??? » : le texte du fichier était limité à 224 caractères, un jeu
  latin occidental. Il ne l'est plus, et l'écran et le papier disent
  maintenant la même chose. Le texte d'un PDF exporté se copie aussi
  proprement dans un lecteur, accents compris. Les albums déjà composés
  s'exportent à l'identique : rien ne bouge tant que rien ne sortait du jeu
  d'avant.
- **Une photo ne peut plus effacer une page de texte.** Envoyer une photo sur
  la planche voisine (⌘⇧flèche) quand celle-ci était une page de texte, la
  page de garde ou le colophon transformait la page en planche photo et son
  texte disparaissait sans le dire. Le déplacement est refusé et la barre
  d'état dit pourquoi. La page de respiration, elle, accepte toujours une
  photo : c'est à ça qu'elle sert.
- **Un dossier de tirages scannés fait un album.** Au-dessus de vingt-cinq
  photos, une photo sans date EXIF, sans GPS et sans étoile était écartée
  comme « parasite », et un dossier où aucune n'en avait — des scans, un
  Takeout sans ses fichiers de dates — était refusé en bloc, sous ce mot-là.
  Le filtre ne s'arme plus que lorsque la majorité des photos porte une
  empreinte d'appareil : sinon il se coupe, le dit, et le livre suit les
  dates de fichier. Et la raison affichée dans le tri ne dit plus
  « parasites » mais « sans empreinte d'appareil ».
- **Une erreur ne laisse plus une fenêtre blanche.** Une levée pendant un
  rendu démontait tout l'écran sans un mot ni un journal. Une frontière
  d'erreur l'attrape désormais : elle dit ce qui s'est passé, ce qui est sur
  le disque et ce qui ne l'est pas, offre le détail à copier pour un
  signalement, et un bouton relance l'écran.
- **Les panneaux sont des dialogues.** Préférences, À propos, Stockage,
  Signaler, Raccourcis et Format s'annoncent comme tels au lecteur d'écran,
  prennent le focus à l'ouverture, le rendent à la fermeture, et tout ce qui
  est derrière eux est inerte le temps qu'ils sont ouverts : Tab n'atteint
  plus les boutons de l'éditeur caché. Le panneau *Format* était le seul
  qu'Échap ne fermait pas ; il le ferme. Et Échap lâche l'objet libre choisi
  en même temps que la case, au lieu de laisser les flèches le déplacer
  pendant qu'on croit tourner les pages.
- **Le livre se réordonne au clavier.** Dans *Planches*, ⌥ flèche déplace la
  planche courante ; Entrée et Espace l'ouvrent. L'ordre des planches ne se
  changeait qu'à la souris.
- **L'écran « album vide » et l'alerte de dossier photo introuvable parlent
  anglais** sur l'interface anglaise ; ils étaient restés en français.
- **Un contrôle avant impression qui ne tourne pas le dit en français**, le
  message brut passant derrière un « Détail technique » comme partout
  ailleurs. Et le panneau *Signaler* ne meurt plus quand le diagnostic de la
  machine est illisible : le rapport part sans lui et le dit.
- **Le rapport de bug partait vide sous Windows.** L'URL de l'issue
  pré-remplie passait par `cmd /C start`, qui coupe au premier `&` : le
  formulaire s'ouvrait sans version, sans système, sans journal — et tout ce
  qui suivait un `&` aurait été exécuté. L'URL est désormais remise au
  navigateur sans interpréteur de commandes entre les deux, et le canal
  n'accepte plus qu'une forme fermée : le formulaire du dépôt suivi d'une
  requête faite de ce que l'application écrit, et rien d'autre.
- **Une vignette ne se lit que dans le cache.** `thumbs.json` est un fichier
  du dossier de l'album, donc un fichier qui se partage ; une valeur
  `../../…` y était suivie telle quelle par six lecteurs. Un nom de vignette
  est un nom de fichier, ou il est refusé.

### Sécurité

- La fenêtre ne nomme plus un chemin du disque : la boîte « Enregistrer le
  PDF » est ouverte par le moteur, qui écrit là où vous avez cliqué et nulle
  part ailleurs, et l'import depuis Photos n'écrit que dans le dossier qu'il
  a lui-même proposé, sous Images › Colophon.

- La chaîne de release refuse un tag dont la version ne serait pas celle des
  quatre fichiers qui la portent, et un CHANGELOG sans section pour elle ;
  elle passe le gate avant de construire quoi que ce soit. Le gate construit
  désormais le bundle de l'interface, garde le Composer sous et sur le seuil
  du petit dossier, et un workflow hebdomadaire lit les avis de sécurité des
  crates. Windows ne sort plus qu'un installeur MSI, donc une seule forme de
  mise à jour ; macOS exige 11.0, ce que la photothèque exigeait déjà.
- **Composer un dossier déjà composé n'écrase plus l'album.** Le même
  dossier de photos résolvait le même dossier de sortie, et la nouvelle
  composition réécrivait l'album qu'on avait édité à la main — titre,
  légendes, recadrages — avec pour seul filet une sauvegarde d'un pas. Un
  album ne s'écrit plus jamais là où un album est : le second se pose à
  côté. Et l'écran de création le demande avant : « Un album a déjà été
  composé depuis ce dossier. Le rouvrir ? ». Le nom du dossier de sortie
  ne dépend plus d'un algorithme que Rust ne garantit pas stable d'une
  version à l'autre.
- **L'écran montre ce que le livre imprime.** Un caractère que la police du
  livre ne dessine pas s'imprime « ? » ; l'écran, lui, retombait sur une
  autre police et montrait la légende parfaite. Le moteur nomme désormais ces
  caractères (quatorzième compteur du linter, `caractere_absent`, qui
  avertit et ne décide pas), l'éditeur les dessine « ? » comme le PDF, dans
  les deux rendus et dans les coupures de ligne, et le panneau *Format*
  comme *Envoi* les listent : « Cette police ne dessine pas 3 caractères de
  l'album, imprimés « ? » : « ż », « ę », « ź » ».

## [0.9.0] - 2026-08-17

Première version candidate publique. Le moteur, l'éditeur et l'export sont
là ; il manque la signature des binaires, la mise à jour automatique et
l'icône définitive.

### Ajouté

- **Trois propositions au lieu d'une.** Le même dossier donne trois albums
  qui diffèrent par le rythme et la longueur, composés d'une seule analyse.
  L'écran de fin de composition devient un écran de choix, et les deux
  propositions écartées restent récupérables jusqu'à la première retouche.
- **Page de colophon.** Une dernière page discrète : photos retenues sur
  photos lues, période couverte, villes traversées, appareils utilisés,
  format et papier. Activée par défaut, retirable d'un clic depuis Envoi.
  Elle ne porte jamais un chemin, une coordonnée ni une légende.
- **Aperçu fidèle (⇧⌘P).** La vue Livre lit le PDF plutôt que de le
  redessiner : ce qui est à l'écran est le fichier, glyphes et rognages
  compris. Rendu par pdf.js, sans réseau.
- **Panneau Stockage** (Fichier → Stockage…) : ce que l'application a écrit
  sur le disque, album par album, avec la suppression et la purge des
  caches de vignettes. Les photos d'origine ne sont jamais touchées.
- **Rendre une planche à l'automatique.** Le cadenas avait une porte
  d'entrée sans sortie ; la planche reprend la composition proposée au
  départ, et la mesure de reprise cesse de la compter.
- **Titre d'album modifiable** depuis la barre, la couverture suivant tant
  qu'elle n'a pas de titre à elle.
- **Écran À propos** : version, licence GPL-3.0, notices des licences
  tierces embarquées, et l'attribution GeoNames qu'exige la CC BY 4.0.

### Modifié

- Politique de sécurité de contenu réelle à la place de l'absence de
  politique : plus rien ne peut être chargé depuis le réseau.
- Le linter passe désormais les trois propositions, sur les trois jeux de
  référence et les six formats.

### Sécurité

- La commande qui supprime un album ne peut atteindre qu'un enfant direct du
  dossier de données, liens symboliques résolus des deux côtés. Un dossier
  de photos n'est jamais atteignable depuis l'application.
