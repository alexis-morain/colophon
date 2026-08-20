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

2. Lancer le harnais sur l'album voulu, **au premier plan** :

   ```bash
   COLOPHON_ALBUM=$PWD/.albums/mesure-200 npm --prefix crates/colophon-app run dev
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

**Relevé de référence, état d'avant le port : pas encore pris.** Le port de
2.3 ne commence pas avant qu'il le soit : c'est la seule contrainte d'ordre
que la mesure impose, et elle n'est pas négociable, un « avant » ne se
rattrape jamais après.

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
