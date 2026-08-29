# Journal des versions

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et
la numérotation le [versionnage sémantique](https://semver.org/lang/fr/).

Une release par semaine le premier mois qui suit le lancement. Les binaires
macOS et Windows, leurs empreintes SHA-256 et ces notes sont publiés ensemble
sur la page des releases.

## [Non publié]

### Ajouté

- **Choisir la police du livre.** Dans *Format*, à côté du format de page :
  les polices installées sur votre machine, groupées par famille, avec un
  champ pour filtrer. Une police pour tout le livre — légendes, titres de
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

### Corrigé

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
