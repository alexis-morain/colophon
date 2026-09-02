# Colophon, pilote technique

Ce fichier possède la **technique** : le moteur, l'app, le gate, les pièges de code,
l'architecture. La **gestion** (le cap, la publication, les fournisseurs, le verdict
papier, le lancement, les arbitrages produit) vit hors du dépôt, dans
`~/Documents/08_IA/02_Outputs/colophon/CLAUDE.md`. Une session cloud ne voit que ce
fichier : tout ce qu'il faut pour coder est ici, et rien d'autre n'est supposé.

Le glossaire du domaine est `CONTEXT.md`. Un synonyme dans un diff est un rapport de bug
qui attend.

Logiciel libre d'albums photo : un dossier en entrée, composition automatique, tout
modifiable, PDF prêt à imprimer. Tauri 2 (Rust + React), GPL-3.0, version 0.9.0.

## Si tu es une session cloud, commence par ça

```bash
./scripts/install-cloud-deps.sh
```

Il pose deux choses que l'image n'a pas, et sans lesquelles le gate ment :

- les **dépendances système de Tauri** (webkit2gtk et compagnie), sans quoi
  `cargo build` échoue sur `colophon-app` et rien d'autre ne se lit dans l'erreur ;
- les **paquets npm** de `crates/colophon-app`. Mesuré le 27/08 : sans `node_modules`,
  `npx tsc --noEmit` sort 1088 erreurs de résolution de modules, aucune ne parlant d'un
  type du projet, et `vitest` ne démarre jamais. Le gate rend 2 sans que rien du projet
  soit en cause. La CI ne voit pas ce piège, elle fait son `npm ci` en étape séparée.

Le script ne sort jamais en échec et se saute tout seul partout ailleurs qu'en session
cloud. Compter environ quatre minutes pour le premier `cargo build --release` derrière.

Il n'est **pas** branché en hook : ce dépôt n'exécute rien tout seul au démarrage d'une
session. On l'appelle, explicitement, et ça se voit.

## Le gate

`./scripts/check.sh` est la seule définition de ce qui est vert. La CI ne fait que
l'installer et le lancer, sur macOS, Ubuntu et Windows. Une règle qui n'existerait que
dans la CI serait une règle que personne ne peut vérifier avant de pousser.

Un seul saut est légitime hors du Mac, et le script l'annonce lui-même : **`pdf-png`**,
qui dépend de `sips`, donc de macOS. Tout autre saut est un échec, pas une tolérance.
Le linter d'albums, lui, ne se saute plus jamais : fiches absentes, gate rouge.

### Le linter tourne partout, les photos ne voyagent pas

Le linter recompose les trois jeux de référence avec le code du jour, puis les audite :
c'est lui qui attrape une régression du Composer avant qu'elle ne parte. Les photos
(5,7 Go) restent sur le Mac ; ce qui voyage, c'est leur relevé — les fiches versionnées
de `crates/colophon-core/fiches/`, régénérées par `scripts/fiches.sh` quand l'analyse
change. Sans les photos, `check.sh` compose depuis les fiches (`--depuis-fiches`, qui
s'arrête avant les pixels et le dit : ni vignettes, ni PDF, ni couverture) et audite
pareil — l'audit lit le `releve.json` posé à côté de l'album, et **refuse de noter**
quand il n'a ni photos ni relevé, plutôt que de laisser le compteur de résolution muet.

Sur le Mac, `check.sh` prouve à chaque passage que les deux chemins rendent le même
`album.json` à l'octet (`root` excepté, normalisé par `scripts/identite-fiches.py`), la
même `curation.json`, et le même verdict de linter, compteur par compteur. Cette
identité est le test de la fonctionnalité **et** le test de fraîcheur des fiches, comme
la fixture de scène l'est pour le dump de géométrie : une fiche qui a vieilli rougit là,
avec `./scripts/fiches.sh` pour remède.

### Le régime de fusion

Le vert des trois OS suffit pour tout, et la fusion depuis le téléphone est légitime,
y compris pour une PR qui touche `layout`, `gabarit`, `audit`, `pipeline`, `prevol` ou
les seuils. (L'ancien régime 1 — `check.sh` local avant fusion pour ces fichiers — est
aboli depuis que le gate est portable, 27/08.)

## La vague en cours

**2.6, la page qui tourne, est close — et la vague 2 avec elle (28/08).** Session 1
livrée le 27/08 : le geste au coin, le clavier, le mouvement réduit, le *fait quand*
tenu sur ses cinq points et mesuré. Les sept verdicts humains du bundle sont tombés le
28/08, tous favorables : la session 2 n'avait rien à coder, et **l'état actuel est
figé** — `DUREE_TOUR`/`DUREE_MIN` restent, une demande pendant un tour reste avalée, la
courbure reste un ombrage (**la lamelle est morte, l'audit de licence de courbure ne
sera jamais dû**), pas de chantier tactile. Le geste vit dans l'aperçu fidèle, **jamais
dans l'éditeur** — la seule surface de lecture du projet, les autres vues étant une
grille, une table lumineuse et une revue photo par photo.

**La feuille**, et c'est le modèle du livre : elle porte au recto la page de droite de la
planche N, au verso la page de gauche de la planche N + 1 — donc un tour ne demande que
les deux planches qu'il joint. `feuille.ts` décide (faces, coin, relâchement, courbe),
`raster.ts` dessine chaque page une fois dans un canvas que personne ne monte,
`Feuilletage.tsx` assemble quatre morceaux par `drawImage`. **Ce qui bouge est une image
du PDF, jamais un redessin de la scène.** Trois règles à ne pas défaire : sans les images
des deux planches, aucune feuille ne se monte et le changement se fait sec ; une feuille
en vol garde la main jusqu'au bout, et un saut la retire ; `prefers-reduced-motion`
supprime le mouvement au lieu de le raccourcir. La couverture ne tourne pas — feuille à
plat, autre fichier, dos au milieu.

**La vague 3 est close** (3.1, 3.2 et 3.3 dans `main`). 3.1 a fait de `focal` un point
de l'image, invariant au ratio ; 3.2 est la bascule, qui en est le premier usage ; 3.3
retire le verdict de la reprise à travers une bascule (« non mesurable ») au lieu de
compter les replis machine comme des mains. **La vague 4 est close sauf 4.2**, différée
(compenser le papier sans épreuve imprimée serait deviner). **5.1 et 5.2 sont dans
`main`** (les sidecars Takeout et la photothèque, voir « Le moteur » et « La
photothèque du Mac ») ; 5.3 (RAW, audit de licence d'abord) reste.

**6.1 est close, ses quatre sessions faites.** s1 : le moteur *lit* les polices du
système. s2 : il sait **sortir une face de son fichier** (`Face::extraire`, chirurgie
de table, jamais de glyphe) — les deux étaient de capacité, le PDF ne bougeait pas d'un
octet. s3 : **le composite**, où chaque octet de chaque PDF change et pas un pixel de
l'image. s4 : **la police voyage avec l'album** (voir « La police de l'album »).
Le sous-ensemblage a été reporté avec sa mesure, et il n'est entré dans aucune des
quatre. Les deux notes de décision de la vague ne bloquent que 6.2.


Vagues 0 et 1 closes. **Verdict de 2.5 : le défaut reste `dom`**, gravé dans `rendu.ts`
et `scripts/mesure-rendu.md`, dettes canvas au parking lot. Une bascule future resterait
un commit qui ne fait que ça. VoiceOver et le rang via le menu natif : entendus et
validés le 28/08 avec le questionnaire du bundle.

## Le moteur

Chaîne : scan (JPEG, PNG, **HEIC via ImageIO, macOS seulement**), analyse (dHash + pHash
DCT, netteté, exposition, visages), curation, Composer, PDF aperçu + 300 dpi + couverture.
Hors macOS, `heic::system()` rend `None` et `scan.rs` compte `skipped_heic` : rien ne casse.

**Le catalogue a un seuil d'entrée** : le banc (`scripts/banc-gabarits.sh`) a retenu 186
générés sur 1893 dans `gabarit::RETENUS` ; `offerts()` = historique + retenus (387 au
dump), le Composer reste sur l'historique, `spec()` reparse tout nom `g_*`.

**Les sidecars Takeout comblent ce que l'EXIF a tu** (`meta.rs`, 5.1) : liste fermée
d'orthographes — jamais un listage de dossier —, l'EXIF gagne toujours, la date de
sidecar est un epoch UTC assumé naïf et dite fiable, `(0,0)` refusé, `favorited` et
`geoDataExif` jamais lus. `-edited` gagne dans un Takeout seulement (la base porte un
sidecar) et l'originale entre dans `curation.json` en `originale_editee`, portée par le
relevé. Un cache de sondes par photo et par processus paie les dix appelants de
`meta::read` (chrono ×1,03 ; ×1,72 sans lui).

**`font.rs` lit les faces du monde, et n'en embarque toujours qu'une.** `Face::parse`
(données, index) rend nom PostScript, nom lisible, métriques, genre (`glyf`/`cff`),
`variable`, et un **verdict codé** — `illisible`, `embarquement_interdit`,
`bitmap_seulement`, `cmap_illisible` — parce que le refus s'affichera un jour avec sa
raison. Une face refusée **se nomme quand même** : seule l'illisible perd son nom.
`installed()` marche les dossiers de la plate-forme aux manières de `scan.rs` (symlinks
non suivis, cachés ignorés, tri par chemin puis rang) et rend **toutes** les faces,
refusées comprises. **Jamais un glyphe n'est lu** : `lire_tables` ne tire du disque que
les huit tables de `TABLES_LUES`, la présence de `glyf`/`CFF `/`CFF2` venant du
répertoire — 787 faces d'un Mac de série en 29 ms, 4,2 % des octets. `metrics()` passe
par le même lecteur, `text_width_mm` et l'aval ne bougent pas. Trois pièges mesurés le
28/08 : `CFF2` est du CFF (sinon neuf faces indiennes tombent), un refus bitmap se
décide sur les contours **avant** les tables communes (une police `bdat` n'a pas de
`head`), et le nom se choisit **en anglais d'abord** (sinon Times s'appelle
« Times 標準體 »). La règle bitmap est « aucune table de contour », jamais « une strike
présente » : cette dernière refuserait Courier New, Monaco, Geneva et Cochin.

**Une face s'ouvre pour écrire, et alors elle garde ses octets** (`font::Embarquee`).
`Face` lit un fichier et le lâche, ce que veut la découverte ; composer une ligne est le
besoin inverse — chaque caractère demande un glyphe au `cmap` et sa chasse à `hmtx`, et
le PDF réclame le fichier lui-même. **C'est le seul endroit où une chaîne devient des
glyphes** : `text_width_mm` et l'émetteur y passent tous les deux, sinon l'album serait
mesuré sur un jeu de chasses et dessiné sur un autre. La règle de substitution y vit
aussi : un caractère que la face ne dessine pas devient `?`, jamais la case vide du
`.notdef` ; une face qui ne dessine pas `?` le laisse tomber. **WinAnsi a quitté le
projet** avec `/Widths` : plus de table de chasses par code, plus d'échappement octal.

**La police de l'album voyage avec lui, et rien ne cherche jamais une police par son
nom** (6.1 s4). `album.police` est **additif, le schéma reste à 2** — le précédent est
`reglages` : absent veut dire « la face du projet », donc aucune migration et un vieil
album s'ouvre tel quel. Il porte le **nom du fichier** posé à côté d'`album.json`, et ce
nom n'a que deux valeurs (`police.ttf` pour du `glyf`, `police.otf` pour du CFF) : un
`album.json` est réparable à la main, donc `dir.join(ce qu'il dit)` serait une traversée
de chemin. Les octets posés sont ceux de `Face::extraire`, **jamais le fichier système
recopié** — Helvetica Neue sort à 14 % de sa `.ttc`. `font::face_album(dir, fichier)`
est la seule porte : elle rend la face **et** `defaut`, le code d'un fichier nommé et
introuvable. **Ce cas ne fait jamais échouer un export** : l'album sort dans la face du
projet et l'écran le dit, en haut d'Envoi comme dans le panneau *Format*.

**Le dossier qui compte est celui d'`album.json`, jamais `album.root`** (les photos).
`PdfWriter::new(album, dir)` le prend, `print`, `cover` et `build` l'ont tous en main.
`Ecrivain` passe alors en `Cow::Owned`, et **toute coupure de ligne se mesure sur la
face du document** (`Ecrivain::largeur_mm`) : `cover.rs` pour le dos et la quatrième,
`Scene::of_avec` pour la page de garde, dont le titre rétrécit jusqu'à tenir — mesuré
dans une face et dessiné dans une autre, il sortirait du massicot. `Scene::of` garde la
face du projet : le linter, le prévol et le dump raisonnent sur une planche, pas sur un
rendu. **Une recomposition détruit tout champ que `BuildOptions` ne porte pas** :
`police` y est, comme `reglages` et `densite`.

**L'app mesure les octets de l'album, jamais une police installée** (`font.ts`). La
commande `police_octets` rend les octets que l'émetteur embarquera, `chargerFace` les
enregistre en `FontFace` sous la famille interne **`colophon-album`**, et `--font-book`
comme le canvas lisent cette pile. La parité est vraie **par construction** : mêmes
octets des deux côtés. Le crénage et les ligatures sont coupés des deux côtés
(`featureSettings`, `ctx.fontKerning`), le moteur n'en dessinant aucun. Mesuré le
29/08 : l'écart écran/moteur est de 1,6·10⁻⁵ mm pour une borne de 6,4·10⁻² mm — et
nommer la face **installée** avec le crénage du navigateur décale de 0,067 mm sur une
ligne de 74,8 mm. Le sélecteur vit dans le panneau *Format*, montre les faces refusées
grisées avec leur raison (`police.ts`, sur le modèle de `reasons.ts`), et le
« Regular » final s'élague **à l'écran** : le moteur rend ce que le fichier déclare.
Il offre **dix familles, une par voix** (`police::selection`, une liste ordonnée de
noms connus dont la première présente gagne), les 787 faces restant derrière
« toutes les polices » — cacher une police installée serait le défaut que ce panneau
existe pour éviter. **Chaque nom est écrit dans ses propres octets** (`specimen.ts`,
commande `police_apercu`, même extraction que le choix mais sans rien poser à côté
de l'album), crénage et ligatures coupés comme dans `font.ts` : un spécimen montre
ce que le livre imprimera. Deux plafonds, et ils sont la raison du compteur — 4 Mo
par face, 24 Mo en tout, au-delà de quoi le nom reste dans la police de l'interface :
une face CJK sortie de sa collection pèse 28,6 Mo et la liste en demande des dizaines.

**Les notes de l'utilisateur entrent dans le score** (`meta.rs`) : une photo rejetée sort
avant comparaison, une étoilée vaut ×1,18 par étoile. `album.json.bak` à chaque sauvegarde.


**Le Composer garantit** : jamais un portrait dans une case paysage (écart ≤ 1,4), les
visages à 4 % des bords au moins, jamais deux quasi-doublons sur une planche, une
ouverture au quartile haut, jamais quatre gabarits d'affilée. Plancher 250 ppi visé,
**pas garanti** : l'audit en tolère trois, le prévol aucun.

**La bascule (`core::bascule`) n'est pas une recomposition.** `recompose_album` rebâtit
tout ce qui n'est pas épinglé ; la bascule ne rebâtit rien — mêmes planches, même ordre,
mêmes photos, mêmes recadrages —, elle ne change que `trim_mm` et le `template` d'une
planche devenue inapte, et jamais vers une capacité plus basse (une photo perdue ne se
voit pas, une cellule trahie se voit et se change). Elle ne décode aucune photo : les
tailles viennent du relevé, sinon des en-têtes des originaux. Elle n'écrit rien côté app,
le moteur rendant un album que l'éditeur applique par son historique — d'où ⌘Z. Son bilan
nomme d'abord les photos passées sous 250 ppi, seul dégât qu'aucune main ne rattrape.
**Pas de « proposition à côté »** (décision 3.3) : ⌘Z côté app, `album.json.bak` côté CLI
et `album.<id>.json` pour les variantes occupent déjà les trois places — un quatrième
mécanisme serait une chose de plus à expliquer. Après une bascule, `--reprise` rend
`"non mesurable"` (champ `bascule`, les deux formats nommés) : les replis machine se
confondraient avec des mains, le verdict se retire, les faits restent, `ok` reste vrai.
**L'aptitude d'un gabarit a une seule définition**, `gabarit::apte` / `gabarit::trahison` ;
le sélecteur, le cycle clavier et la bascule la lisent.

Titres de chapitre depuis le GPS (`core::places`, GeoNames CC-BY) ; seul
`DateTimeOriginal` date un chapitre. **Deux rythmes** choisis au lancement, stockés dans
`album.json`, relus à la recomposition (`layout::Densite`). **Une composition donne trois
propositions** (`build::variantes_offertes`), en `album.<id>.json`, effacées au premier
enregistrement. **Deux planches ordinaires encadrent le livre** (`album.colophon`,
décochables depuis Envoi) : colophon en queue, page de garde en tête, hors chapitres.

## La photothèque du Mac

**Elle n'entre jamais dans le moteur : elle produit un dossier** (`app/photos.m`,
`app/photos.rs`, `app/bibliotheque.ts`). L'import écrit les photographies choisies dans
un dossier visible, et `build_album_from_folder` le compose comme n'importe quel dossier
du Finder. Ni `scan.rs`, ni `meta.rs`, ni la curation, ni le linter, ni le prévol
n'apprennent qu'Apple Photos existe, et le gate reste vert sur Ubuntu et Windows **sans
un `#[cfg]` de plus dans le moteur**. Ce choix se payait d'une copie ; la mesure du 02/09
l'a rendu gratuit : `writeDataForAssetResource:` rend le fichier d'origine **à l'octet**
et clone les blocs APFS, 219 Mo de photographies pour **2 Mo de disque réel**.

**Le pont est un fichier Objective-C compilé par `cc`**, la raison de `heic.rs` : les
liaisons à la main gardent l'arbre vide, et surtout les blocs restent dans la langue qui
les possède, repliés derrière un `dispatch_semaphore`. La frontière Rust ne voit que du C
plat et cinq fonctions. Toute la politique (ordre, noms, refus du réseau, annulation,
rapport) vit côté Rust.

**Le piège central : PhotoKit ne lève jamais pour un défaut d'accès, il rend une liste
vide.** Trois causes distinctes rendent cette même liste vide, d'où `Etat` à trois
branches et non un booléen : *pas encore demandé*, *autorisé mais bibliothèque système
injoignable*, *réellement vide*. Le deuxième est un état d'utilisateur réel, mesuré sur
cette machine : PhotoKit ne lit **que** la bibliothèque système (`SystemLibraryPath` de
`group.com.apple.photolibraryd.private`), pas celle qui est ouverte dans Photos.app, et
un renommage de photothèque laisse le chemin mort — autorisation accordée, échecs
`NSXPCConnection` en boucle, zéro album. La phrase de cet état nomme le chemin et la
case à cocher ; sans elle on cherche le défaut dans Colophon pendant une heure.
`bibliotheque.test.ts` tient la distinction, et il mord (vérifié par mutation).

**Trois règles à ne pas défaire.** Le réseau est refusé au premier passage
(`networkAccessAllowed = NO`) : ce qui est resté dans iCloud se compte, se **nomme**, et
ne se télécharge pas sans un oui devant un chiffre. Toute requête porte une garde de
délai, parce qu'en signature ad-hoc — la nôtre — une requête PhotoKit peut ne jamais
rendre la main. Et les noms sont préfixés par le rang (`0001-IMG_2193.jpg`) : dans une
bibliothèque, `originalFilename` collisionne massivement, et un suffixe « (1) »
rejouerait le dégât que 5.1 répare. Le rapport `import.json` est posé à côté des photos,
et `scan.rs` ignore déjà `json`.

**`PHAccessLevelReadWrite` n'est pas un choix** : l'énumération n'offre que `AddOnly` et
`ReadWrite`, il n'existe aucun niveau lecture seule, et `AddOnly` ne lit rien. macOS
annoncera donc que Colophon peut modifier la photothèque, ce qui est faux ; la
`NSPhotoLibraryUsageDescription` de l'`Info.plist` est le seul endroit où la vérité se
rétablit, et c'est elle que l'utilisateur lit.

## La scène, source unique de ce que porte une planche

`Scene::of(planche, géométrie)` rend des objets : rectangle, profondeur (l'indice), rang
de lecture, rôle codé (`photo`, `photo_caption`, `chapter_caption`, `text`). L'émetteur,
le tirage 300 dpi, le linter, le prévol **et l'écran** la lisent tous. **Dérivée, jamais
stockée** : `album.json` n'a pas bougé, donc aucune migration. `garde`, `texte` et
`colophon` sont un rôle `Text` aux lignes déjà mises en page. Un texte est placé par sa
ligne de base (`at`) et couvre son encre mesurée (`rect`), deux choses distinctes, et
`caption_box` reste le proxy de placement du dump. **Pour un gabarit-candidat, c'est
toujours `slots_for`** ; pour ce que porte une planche, c'est la scène.

`scene.ts::sceneOf` en est le miroir TypeScript, millimètres, origine en haut à gauche.
**Deux rendus consomment la même scène**, DOM et `SceneCanvas`, derrière `rendu.ts`,
défaut `dom`. Les quatre gestes passent par `scene.ts::hitTest` ; un recadrage en vol se
substitue **dans la scène** (`avecRecadrage`), donc aucun rendu ne sait ce qu'est un
brouillon. **`SceneProxies` pose une boîte focusable par objet**, dans l'ordre de lecture,
nommée depuis le rôle via `i18n.ts`, `pointer-events: none`, clippée au rognage. **Les
deux rendus donnent le même arbre d'accessibilité** (37 objets nommés, listes identiques
sur neuf planches) : ce qu'un rendu peint est du décor, marqué objet par objet, jamais un
`aria-hidden` sur le conteneur, deux champs de saisie y vivent.

**La géométrie a une seule source** (`gabarit.rs`, dump lu par le TS via `geometrie.ts`).
Restent portés et sous parité : fenêtres de recadrage, `coverSheet`, `gardeLayout`, et
l'assemblage de la scène, épinglé objet par objet sur neuf planches × six formats. Seule
l'encre mesurée échappe, faute de fonte sous Vitest, et court la mesure synthétique des
deux côtés. **Les deux fixtures se régénèrent** quand le dump ou la scène change
(`--dump-geometry`, `scripts/fixture-scene.sh`) : leur fraîcheur est un test.

## L'app

Recadrage, tiroir, table lumineuse ⌘3, badge « éditée » et cadenas ⌘L, « rendre à
l'automatique », recomposition préservante, légendes, texte, couverture, **Envoi ⌘4**
(qui offre le verdict après un export), bilan de choix, revue clavier, Stockage, À propos,
**Préférences ⌘,** (FR/EN et le rendu des planches, `i18n.ts`, sans redémarrage),
**aperçu fidèle ⇧⌘P** (pdf.js).

**Le clavier garde sa place quand la page tourne** : la couche est démontée à chaque tour
(`key={index}` rejoue l'animation), donc la mémoire du rang vit hors du composant ; au
bout de l'ordre de lecture, la flèche tourne la planche. **Un champ ouvert depuis une
boîte rend le focus à cette boîte** : le rang survit au blur vers le champ, et Entrée
valide avec `preventDefault`, sans lui l'action par défaut rouvrait le champ. **Le canvas
avale son clic résiduel**, sans quoi le papier désélectionnait dans le même souffle. **La
ligne de statut est vivante** (`role="status"`, les quatre pieds) et **la table lumineuse
se parcourt au clavier** (un arrêt de tabulation, flèches, verticales d'une rangée).
**La légende proposée** : champ vide, fantôme gris (`legende::proposition`), Tab la pose
et marque `edited`. Badges de case : « N ppi », « sombre », infobulle = remède.

**Le sélecteur de gabarits montre des dispositions** (`gabarit.ts`) : verso, bande de
légende et forme de cellule repliés, une entrée par disposition, groupées par nombre
de photos — les 171 gabarits qu'une planche de quatre photos peut prendre deviennent
au plus 23 entrées, et chacune porte un nom français au lieu du `g_1x2f_1x2f` que le
catalogue généré affichait. La variante réellement posée est celle de plus faible
trahison, **sans bande d'abord** (le Composer n'en pose aucune) : `gabarits_compatibles`
remonte donc `[nom, trahison]`, l'aptitude gardant sa définition unique côté moteur.
La grammaire des noms générés est relue côté app **pour le seul libellé**, et
`gabarit.test.ts` exige qu'aucun nom du dump n'y échappe, que la disposition déclare
autant de cases que le dump en pose, et qu'elle les répartisse du même côté du pli —
c'est ce test qui tient la table des familles historiques, seul endroit de l'app qui
redit une forme du moteur. G et ⇧G lisent la même liste.

Menu natif (`menu.ts`), barre en trois zones, couverture = cellule zéro de Planches,
export uniquement par Envoi. **Toute commande passe par la table `raw` d'App et
`menu.ts`** ; les cinq panneaux s'excluent. **Aide puis Signaler** (`signaler.ts`) :
rapport entier visible avant envoi, jamais un chemin ni une légende.

**DA** : piste A, chrome clair sur blanc, plus un mode sombre qui suit le système, un
bloc de tokens, contraste vérifié. Neutres froids, terracotta `#b04a1f` / `#e07a4a`, zéro
serif. `--ink-rgb` = encre du papier, `--chip-*` = pastilles sur photo, `--salle-*` = la
salle sombre.

## Le PDF

**L'export se déclare PDF/X-4 et PDF/A-2b** (`core::pdfx`) : OutputIntent sRGB, XMP,
`/Trapped`, `/ID`, en-tête 1.6. Aucun validateur gratuit ne certifie X-4 : la mesure est
PDF/A-2b (veraPDF) plus cinq tests Rust, le verdict X-4 revient au prévol imprimeur.
**Le PDF est reproductible** : `SOURCE_DATE_EPOCH` honoré, deux exports du même album
sont identiques à l'octet.

**Le texte est écrit en composite** (`/Type0` sous Identity-H, descendant
`/CIDFontType2`, `/CIDToGIDMap /Identity`, plus `/ToUnicode`) : le code du flux **est**
le glyphe, écrit en chaîne hexadécimale, et rien ne dépend plus d'une table d'encodage.
Une légende porte donc **tout ce que la face sait dessiner**, et plus les 224 codes d'un
encodage à un octet. La face entre **entière** : aucun préfixe de sous-ensemble sur son
nom, aucun `/CIDSet` — les deux annonceraient un sous-ensemble qui n'existe pas. Seuls
`/W` et `/ToUnicode` sont restreints aux glyphes **dessinés**, accumulés pendant le
dessin dans `font::Utilises`. Trois conséquences à ne pas défaire : l'accumulateur est
**ordonné** (une carte de hachage rendrait un fichier différent d'un export à l'autre) ;
les objets de police sont **réservés à la construction et écrits à `save()`**, les
glyphes n'étant connus qu'à la dernière planche ; et l'écrivain **descend jusqu'aux
appelants**, `cover.rs` écrivant son flux hors du writer — un accumulateur logé dans le
writer perdrait tout le texte du dos sans que rien le voie.

**Trois PDF** : `album.pdf` = aperçu vignettes, jamais imprimé, mais c'est lui que lit
l'aperçu fidèle ; `--print` = 300 dpi, rien ne court-circuite `print_scale` ; `--cover` =
la feuille à plat, une par profil. **Ce qui doit survivre au massicot se mesure depuis la
coupe**, et il n'y a plus qu'une implémentation, `scene::distance_to_trim`.

`--audit` : dix compteurs, 18/18 verts (3 jeux × 6 formats), sur les trois propositions de
chaque jeu. `--reprise` : part des planches corrigées à la main contre
`album.origin.json` ; sous 10 % bon, jusqu'à 30 % à surveiller, au-delà rédhibitoire.
`--prevol --profil <id>` : bloquants contre un `PrinterProfile`.

## Pièges connus

**Les chaînes nées dans le moteur restent françaises sur l'écran anglais** (défauts du
prévol, fiches, rythmes, formats, variantes) : le jour où ça compte, des codes côté
moteur et le libellé côté app. **Vitest tourne sans `navigator`**, langue par défaut
anglaise : un test qui affirme du français pose `setLangue("fr")`. Sous
`pipeline::PETIT_DOSSIER` (25 photos), la curation se limite aux rejets certains.

**`album.origin.json` ne se réécrit jamais**, c'est la référence de `--reprise` :
composer dans un dossier déjà utilisé mesurerait l'album du jour contre une vieille
proposition. `check.sh` efface avant. **Les seuils du linter sont co-réglés avec le
Composer** (250 ppi, écart 1,4, doublons 24 bits dHash / 8 pHash / 180 s, couleur ≤ 20)
dans `audit.rs`, importés par `layout.rs`.

**Les actifs sous licence gardent leur licence à côté** : `colophon-core/assets/` (OFL,
ICC sRGB, GeoNames **CC-BY, attribution obligatoire**) ; l'écran À propos porte les trois.

**Le serveur de dev écrase le vrai `album.json`** (POST `/__dev/album`) : copies jetables.
**Recharger la page après toute édition de source**, le fast refresh Vite corrompt l'état
React. **Vignettes** : nom non devinable, tout lecteur passe par `thumbs.json` ; sous
1600 px une vignette EST l'original, le badge ppi s'y fie. **Mémoire** : pipeline sur
vignettes, l'original ne s'ouvre qu'au rendu.

**Une fenêtre en arrière-plan ne reçoit aucune trame d'animation** : pdf.js n'y résout
jamais sa promesse et `mesure.ts` n'y pose aucun chiffre, vérifier `document.visibilityState`
avant d'accuser le code. **Une fenêtre sans le focus système ne reçoit aucun événement de
focus** (`hasFocus()` faux, les deux harnais) : `focus()` déplace `activeElement`, mais
`focusin` ne part pas et `:focus` ne matche pas. Tout ce qui touche au clavier ou au
chronomètre se vérifie **au premier plan**, jamais au harnais. **Mais une capture d'écran
force une étape de rendu** : sans capture préalable, `ResizeObserver` n'a jamais livré ses
callbacks, l'échelle de la planche vaut 1 et tout relevé DOM est faussé. Capturer, puis
relever. La cure des fenêtres borgnes : instance Brave dédiée, drapeaux anti-occlusion,
focus émulé CDP, `scripts/mesure-cdp.mjs` et `scripts/mesure-rendu.md` § « La cure ».
**pdf.js** : jamais de lambda dans les dépendances de l'effet de rendu. Et **l'aperçu
fidèle ne s'affichera jamais dans un harnais borgne**, capture d'écran ou pas : la
promesse de rendu de pdf.js ne se résout que sur une trame d'animation, qu'une fenêtre
cachée ne reçoit pas. Tout ce qui lit le PDF à l'écran se vérifie sur l'instance Brave de
la cure — `scripts/feuille-cdp.mjs` en est le second pilote, 31 épreuves sur la page qui
tourne, et il se vérifie mordant en mutant le code, comme un test.

**Une bascule charge la géométrie de sa cible avant de s'appliquer.** `geometrie()`
jette pour un format que personne n'a chargé, et c'est le bon choix — mais aucune
frontière d'erreur ne couvre l'arbre React : la levée démonte tout et il ne reste
**qu'une fenêtre blanche**, sans message ni journal. Le dump du format visé se charge
donc pendant l'aperçu (`chargeGeometrieFormat`, **dans le fond perdu de l'album**, pas
celui de l'écran de création qui vaut zéro et n'a jamais la bonne clé), et
`adopterGeometrie` refuse plutôt que de laisser appliquer un album que personne ne
saurait dessiner. L'ancien dump reste en cache, et c'est ce qui fait tenir ⌘Z sur une
bascule. `geometrie.test.ts` tient les quatre cas.

**Une UX ne se valide jamais au seul harnais navigateur** (`.claude/launch.json`).
**Installer le bundle après chaque push** (TCC pour le pilotage à l'écran), 19 Mo.
**Les artefacts de mise à jour n'existent qu'en release** (`tauri.release.conf.json`).

**Un canvas casse l'accessibilité par nature** : les proxies DOM sont posés, et
`SceneProxies` doit rester une fonction de la scène. Le jour où il lit le gabarit ou une
chaîne du moteur, il cesse de servir les deux rendus.

**`album.root` est le dossier des photos, la police est à côté d'`album.json`.** Les
confondre donne un album qui marche tant que les deux coïncident et casse au premier
album rangé ailleurs. Et **l'écran ne nomme jamais une police installée** : ça marche
sur la machine qui l'a, donc le défaut ne se voit qu'ailleurs. Le test qui mord est
`font.test.ts`, qui lit la chaîne posée sur le contexte, pas une constante à côté.

**Jamais un octet écrit sur un original**, la retouche vit dans `album.json`. **Jamais
d'échec silencieux à l'export.**
 **Tout ce qui touche le PDF remesure la conformité**
(cinq tests Rust plus veraPDF) dans la vague où c'est écrit.

**Trois manières de mesurer une photo, et deux qui mentent.** `image::image_dimensions`
perd toute photo HEIC : la seule répartition du projet est `heic::dimensions`. Un en-tête
brut n'est pas orienté alors que `Photo::orig` l'est : passer par `heic::oriente`, sans
quoi toute photo couchée se lit paysage. Et `Releve::lire` recompose ses chemins depuis la
racine, donc une fiche relue porte `jeu/photo.jpg` là où un slot porte `photo.jpg` :
la clé est `Releve::src`. Les trois se sont trouvées en confrontant la bascule depuis les
photos et depuis les fiches ; aucune n'aurait été vue autrement, la troisième rendant un
succès parfait sur zéro travail.

## Décisions à ne pas rouvrir, côté code

Tauri 2 et React. GPL-3.0. `album.json` état unique réparable à la main. Le PDF fait foi.
Aucune image ne traverse le pli. Heuristiques d'abord, IA jamais décisionnaire. Jamais de
résolution sous 250 ppi. Jamais `imazen/heic` (AGPL).

## Commandes

```bash
./scripts/check.sh
```

```bash
./target/release/colophon ~/Pictures/colophon-testsets/corse-2013 -o .albums/corse-2013 --format carre-21 && ./target/release/colophon --audit -o .albums/corse-2013
```

Autres drapeaux : `--print`, `--cover`, `--prevol --profil <id>`, `--densite`,
`--variantes`, `--reprise`, `--bascule <FORMAT> [--essai]`, `--dump-scene`, `--dump-geometry`, `--profils`. Scripts :
`pdfx.sh full`, `install-app.sh`, `fixture-scene.sh`, `notices.sh`, `apercu-fidele.py`,
`banc-gabarits.sh`, `mesure-cdp.mjs`, `feuille-cdp.mjs`, `police-cdp.mjs`. App :
`npm run tauri dev`.

Les deux bancs à la main de la police, sur l'instance Brave de la cure : poser une vraie
face du Mac dans un album sans passer par la fenêtre, puis mesurer la parité écran/papier
contre la référence du moteur.

```bash
COLOPHON_ALBUM=.albums/corse-2013 COLOPHON_FACE="Helvetica Neue Regular" \
  cargo test -p colophon-core --release banc_poser_une_face_du_mac -- --ignored --nocapture
```

```bash
COLOPHON_POLICE=.albums/corse-2013 cargo test -p colophon-core --release \
  banc_parite_ecran_papier -- --ignored --nocapture > /tmp/ref.json
```


## Architecture

Workspace Cargo. **`colophon-core`** : `scan` → `meta` → `thumb` → `analyze` → `face` →
`heic` → `pipeline` (curation) → `layout` (Composer, `Densite`) → `scene` → `pdf` →
`print` → `cover` → `audit` ; `build.rs` enchaîne. À côté : `font`, `icc`, `places` (les
trois actifs), `pdfx`, `reprise`, `log`, `printer`, `prevol`, `colophon` (la page).
**`colophon-cli`** : clap. **`colophon-app`** : React et Vite (`bridge.ts` seule porte,
`album.ts` géométries, `scene.ts` la scène et `hitTest`, `SceneCanvas.tsx` le peintre,
`SceneProxies.tsx` le clavier, `rendu.ts` l'interrupteur, `feuille.ts` le modèle de la
feuille qui tourne, `raster.ts` le PDF en bitmaps, `Feuilletage.tsx` la scène du
feuilletage, `photos.ts` vignettes décodées et badges, `font.ts` la mesure de texte sur
les octets de l'album, `police.ts` les noms et les refus d'une face,
`menu.ts`, `signaler.ts`, `pdfview.tsx`, `reasons.ts`, `icons.tsx`, `recents.ts`) plus la coquille Tauri, marques d'icône dans
`design/marques`.


Chaîne de distribution : `NOTICES.md` généré et embarqué, CSP réelle, CHANGELOG, README,
modèles d'issue, `check.yml` et `release.yml` (binaires, SHA-256, `latest.json`), updater
branché. Reste le parcours de correction.

Le prompt de la session en cours, quand il y en a un : `docs/prompts/en-cours.md`.
Mesures : `docs/mesures/`. Glossaire : `CONTEXT.md`.
