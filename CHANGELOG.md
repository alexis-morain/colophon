# Journal des versions

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et
la numérotation le [versionnage sémantique](https://semver.org/lang/fr/).

Une release par semaine le premier mois qui suit le lancement. Les binaires
macOS et Windows, leurs empreintes SHA-256 et ces notes sont publiés ensemble
sur la page des releases.

## [Non publié]

### Ajouté

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
