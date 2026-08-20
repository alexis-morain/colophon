# Mesurer le rendu, avant et après le port en Canvas

La vague 2 remplace la couche d'affichage de l'éditeur. Une réécriture qui ne
se mesure pas est un pari : ce document dit comment prendre les trois nombres,
de façon à ce que la passe d'après (2.5) les reprenne à l'identique.

**Ce qu'on mesure, et pas autre chose.** Le port ne touche pas au moteur, donc
chronométrer une composition mesurerait la mauvaise chose. Les trois nombres
sont des nombres d'écran, tous pris entre un rendu React et le pixel peint :

| série | ce qu'elle chronomètre |
|---|---|
| `planche.premiere` | un album est adopté → sa première planche est peinte |
| `planche.suivante` | le lecteur change de planche → la suivante est peinte |
| `recadrage.trame` | un événement de glisser → la trame recadrée est peinte |

Chaque mesure se ferme sur **deux** `requestAnimationFrame` imbriqués : le
premier se déclenche avant que le navigateur peigne, donc s'arrêter là
compterait un travail qui n'est pas encore à l'écran. Et une fenêtre en
arrière-plan ne reçoit aucune trame d'animation — le piège que l'aperçu fidèle
porte déjà — donc une mesure commencée là ne se ferme jamais plutôt que de
poser un chiffre faux. **Mesurez au premier plan.**

**Seul le rapport avant/après fait foi.** Le serveur de dev est plus lent que
le bundle (pas de minification, HMR branché), et l'instrumentation ne vit
qu'en dev : `mesure.ts` se réduit à une branche morte sur
`import.meta.env.DEV`, le bundle n'en porte pas une ligne. Une milliseconde
absolue prise au harnais ne dit donc rien ; le rapport entre deux passes du
même harnais, si.

## La passe

1. Composer les deux albums de mesure si besoin, puis **fermer tout ce qui
   pourrait voler du temps machine** (autre build, autre onglet lourd).

   ```bash
   ./target/release/colophon ~/Pictures/colophon-testsets/corse-2013 -o .albums/mesure-50 --format carre-21
   ```

   L'album long se fabrique en répétant les planches du premier jusqu'à 200 :
   c'est le rendu qu'on mesure, pas la curation, donc un album synthétique est
   exactement aussi valable et beaucoup plus rapide à obtenir. Effacer son
   `album.origin.json` : un album de mesure ne doit ressembler à aucune
   référence de `--reprise`.

2. Lancer le harnais sur l'album voulu, **au premier plan**. Chemins absolus :
   `$PWD` et `--prefix` relatif ne valent que depuis la racine du dépôt, et se
   trompent en silence ailleurs.

   ```bash
   COLOPHON_ALBUM=/Users/alex-pack/Developer/colophon/.albums/mesure-200 npm --prefix /Users/alex-pack/Developer/colophon/crates/colophon-app run dev
   ```

3. Dans la page, faire exactement ceci, dans cet ordre :
   - laisser la première planche s'afficher (c'est `planche.premiere`) ;
   - `window.__mesuresOubli()` pour jeter le bruit du démarrage ;
   - **trente** flèches droite, sans hâte (c'est `planche.suivante`) ;
   - sélectionner une photo, la glisser lentement d'un bord à l'autre de sa
     case, trois fois (c'est `recadrage.trame`).

4. Relever :

   ```js
   copy(JSON.stringify(window.__mesures(), null, 1))
   ```

5. Déposer le relevé en `docs/mesures/<date>-<état>.json`, avec la machine, la
   version et l'état du code. Un chiffre sans son contexte n'est pas
   comparable.

## La fenêtre doit être visible, et ce n'est pas un détail

Une fenêtre qui ne composite pas ne reçoit aucune trame d'animation, donc
aucune mesure ne se ferme. Vérifiez-le avant de commencer, une ligne :

```js
document.visibilityState   // doit répondre "visible"
```

Un onglet en arrière-plan, un pane d'aperçu masqué, une fenêtre derrière une
autre : dans les trois cas `window.__mesures()` reste vide. C'est voulu — un
relevé qui se fermerait sans trame donnerait un chiffre pris avant le pixel —
mais c'est aussi la première chose à vérifier quand rien ne s'affiche.

## Deux fenêtres borgnes, et la seconde a été trouvée le 21/08

Le panneau d'aperçu ne composite pas : `visibilityState` y répond `hidden`,
donc aucune trame n'arrive et aucune mesure ne se ferme. C'est écrit plus
haut. Le navigateur piloté à côté est borgne d'un second œil, découvert en
essayant d'y vérifier le focus :

```js
document.hasFocus()   // faux tant que la fenêtre n'est pas au premier plan
```

Un document qui n'a pas le focus du système ne reçoit **aucun événement de
focus**. `element.focus()` y déplace bien `document.activeElement`, mais ni
`focus` ni `focusin` ne partent, et `:focus` ne matche pas. D'où deux choses
qui se ressemblent et n'en font qu'une : l'anneau de focus qu'on n'a jamais
vu au harnais, et un clavier dont le comportement ne s'y observe pas.

**Les deux passes — le chronomètre et le focus — demandent donc la même
chose : une fenêtre au premier plan, celle du bundle ou celle d'un navigateur
qu'on met devant.** Vérifier les deux lignes avant de commencer, toujours :

```js
document.visibilityState === "visible" && document.hasFocus()
```

## Il n'y a pas de relevé d'avant-port, et c'est délibéré

Ce document a d'abord posé le relevé d'avant le port comme une précondition
dure de 2.3, au motif qu'un « avant » ne se rattrape pas après. C'est vrai en
général, et faux ici, pour une raison précise : **2.3 garde le rendu DOM
vivant derrière un interrupteur** jusqu'à ce que 2.5 tranche. Les deux rendus
coexisteront donc, et 2.5 les mesurera le même jour, sur la même machine, sous
la même charge, avec le même album — une comparaison strictement meilleure
qu'un chiffre vieux de trois semaines pris sur un autre état du code.

L'instrumentation reste en place et la procédure aussi : le jour de 2.5, on
relève une fois par rendu, on bascule l'interrupteur entre les deux, et le
rapport se lit sans rien avoir à retrouver. Le témoin moteur ci-dessous garde
tout son sens, lui : il n'a pas de jumeau à comparer sur place.

## Le témoin moteur

Le port ne doit pas déplacer le moteur d'un centième. Le témoin le prouve :

```bash
time ./target/release/colophon ~/Pictures/colophon-testsets/mauritanie-2019 -o /tmp/temoin --format carre-21
```

Il se relève avant et après, et il doit être identique aux fluctuations de
machine près. S'il bouge, le port a touché ce qu'il ne devait pas toucher.

## La passe au bundle

Une fois par état, à la main, parce qu'aucun pilote de navigateur n'est
installé et qu'en ajouter un pour deux passes coûterait une dépendance, une
licence et une ligne de notices. Installer le bundle
(`./scripts/install-app.sh`), refaire le parcours du point 3, relever. Le
bundle fait foi sur le ressenti ; le harnais fait foi sur la comparaison.
