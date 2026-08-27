// Le feuilletage : la scène où les feuilles tournent, et le seul endroit du
// projet où une animation a le droit d'être remarquée — parce qu'elle est le
// produit. Un album n'est pas un diaporama : c'est un livre, et un livre se
// feuillette.
//
// **Ce qui bouge est une image du PDF, jamais un redessin de la scène.**
// L'aperçu fidèle existe précisément pour que ce qu'on montre soit ce que
// l'imprimeur produira ; une feuille qui se redessinerait en tournant
// mentirait sur le résultat au moment exact où le lecteur regarde le plus. Les
// deux faces de la feuille et les deux moitiés immobiles sortent toutes du
// même bitmap, par `drawImage` : quatre morceaux, deux planches, zéro
// arithmétique de mise en page.
//
// **Où ce geste vit, et où il ne vit pas.** Ici, et dans les surfaces de
// lecture. Jamais dans l'éditeur, où le glisser sert déjà au recadrage : deux
// sens pour le même geste au même endroit est un défaut, pas une richesse. La
// zone de coin est écrite dans `feuille.ts`, une fois, et le curseur comme le
// test de départ la lisent tous les deux là.
//
// **Le geste n'est jamais le seul chemin.** Les flèches, Page haut et Page bas
// font exactement la même chose, par la même mécanique — et un lecteur qui a
// demandé moins de mouvement reçoit un changement sec, sans que rien d'autre
// ne change pour lui.
//
// **La courbure est un ombrage, pas un maillage.** La feuille est un plan qui
// pivote au pli, sous une perspective, avec un voile qui se creuse près de la
// charnière et une ombre portée sur la page qu'elle découvre. Modéliser une
// vraie courbure demanderait WebGL ou une bande de lamelles, et ni l'un ni
// l'autre ne se paie pour quatre cents millisecondes : la règle de la vague
// est que le mouvement sert la lecture, pas qu'il se fasse admirer.

import {
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  Cote,
  Feuille,
  Sens,
  adoucir,
  angle,
  coinTouche,
  COIN,
  dureeRestante,
  estUnClic,
  feuilleDe,
  issue,
  planchesAPrecharger,
  progresDuPointeur,
  relief,
} from "./feuille";
import { t } from "./i18n";
import { echantillon } from "./mesure";
import { Raster, precharger, rasterPret, rasteriser } from "./raster";

/** Ce qu'App peut demander au feuilletage : tourner, et savoir s'il a pris. */
export type Tourneur = {
  /** Lance un tour animé. Rend `false` quand il n'a pas pu — au lecteur de
   *  changer de planche sèchement, ce que fait déjà App. */
  tourner(sens: number): boolean;
};

/** Un tour en cours : la feuille, les deux planches qui l'alimentent, et la
 *  manière dont il a commencé. */
type Tour = {
  feuille: Feuille;
  images: Map<number, Raster>;
  /** Au clavier ou au clic, la feuille se conduit seule jusqu'au bout. */
  auto: boolean;
};

/** Le lecteur qui a demandé moins de mouvement reçoit un changement sec. Lu à
 *  chaque geste plutôt qu'abonné : le réglage change entre deux gestes, jamais
 *  pendant l'un d'eux. */
function mouvementReduit(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function Feuilletage({
  planche,
  total,
  cle,
  largeur,
  onPlanche,
  onErreur,
  ref,
}: {
  /** L'indice de la planche à l'écran, base zéro. */
  planche: number;
  total: number;
  cle: number;
  /** La largeur de la planche double, en pixels CSS. */
  largeur: number;
  onPlanche: (sens: number) => boolean;
  onErreur: (message: string) => void;
  ref?: React.Ref<Tourneur | null>;
}) {
  const [repos, setRepos] = useState<Raster | null>(null);
  const [tour, setTour] = useState<Tour | null>(null);
  const [coinSurvole, setCoinSurvole] = useState<Sens | null>(null);

  const scene = useRef<HTMLDivElement>(null);
  const feuillet = useRef<HTMLDivElement>(null);
  const courbure = useRef<HTMLDivElement>(null);
  const ombre = useRef<HTMLDivElement>(null);
  const trame = useRef<number | null>(null);
  const progres = useRef(0);
  const geste = useRef<Geste | null>(null);

  // La planche à plat. Prise dans le cache sans attendre quand elle y est,
  // ce qui est le cas courant grâce au préchargement ; sinon on garde la
  // dernière dessinée à l'écran le temps du tirage, plutôt que d'ouvrir un
  // trou blanc au milieu d'un livre.
  const pret = rasterPret("album", planche + 1, cle, largeur);
  const affiche = pret ?? repos;

  useEffect(() => {
    if (largeur <= 0 || planche < 0) return;
    if (pret) {
      if (pret !== repos) setRepos(pret);
      return;
    }
    let vivant = true;
    rasteriser("album", planche + 1, cle, largeur).then(
      (r) => vivant && setRepos(r),
      (e) => vivant && onErreur(String(e)),
    );
    return () => {
      vivant = false;
    };
    // `onErreur` change d'identité à chaque rendu de la fenêtre et relancerait
    // le tirage à chaque battement de la ligne de statut.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [planche, cle, largeur, pret, repos]);

  // Les voisines, dessinées avant qu'un doigt se pose. Une fenêtre en
  // arrière-plan n'en dessine aucune (`precharger` le refuse), donc on
  // redemande quand elle revient : c'est aussi le seul moment où précharger
  // vaut quelque chose.
  useEffect(() => {
    if (largeur <= 0) return;
    const lancer = () =>
      precharger(
        "album",
        planchesAPrecharger(planche, total).map((n) => n + 1),
        cle,
        largeur,
      );
    lancer();
    const doc = window.document;
    doc.addEventListener("visibilitychange", lancer);
    return () => doc.removeEventListener("visibilitychange", lancer);
  }, [planche, total, cle, largeur]);

  // ---- le mouvement ------------------------------------------------------

  /** Écrit la position de la feuille dans le DOM, sans repasser par React :
   *  soixante fois par seconde, un rendu React coûterait le mouvement. */
  const appliquer = (p: number, sens: Sens) => {
    progres.current = p;
    const f = feuillet.current;
    if (f) f.style.transform = `rotateY(${angle(p, sens)}deg)`;
    const r = relief(p);
    if (courbure.current) courbure.current.style.opacity = String(r * 0.34);
    if (ombre.current) ombre.current.style.opacity = String(r * 0.3);
  };

  const arreter = () => {
    if (trame.current !== null) cancelAnimationFrame(trame.current);
    trame.current = null;
  };

  /** Finit le mouvement tout seul, de `de` à `vers`, puis fait ce qu'il faut. */
  const animer = (de: number, vers: number, sens: Sens, apres: () => void) => {
    arreter();
    const duree = dureeRestante(de, vers);
    const t0 = performance.now();
    let precedente = t0;
    const pas = (maintenant: number) => {
      echantillon("feuille.trame", maintenant - precedente);
      precedente = maintenant;
      const avance = Math.min(1, (maintenant - t0) / duree);
      appliquer(de + (vers - de) * adoucir(avance), sens);
      if (avance < 1) {
        trame.current = requestAnimationFrame(pas);
      } else {
        trame.current = null;
        apres();
      }
    };
    trame.current = requestAnimationFrame(pas);
  };

  /**
   * Monte la feuille, si un tour est possible et si les deux planches sont
   * déjà dessinées. Sans les images le premier pas du mouvement sauterait, et
   * un mouvement qui saute est pire que pas de mouvement : on rend `false`, et
   * le changement se fait sec.
   */
  const demarrer = (sens: Sens, auto: boolean): boolean => {
    if (tour || mouvementReduit() || largeur <= 0) return false;
    const feuille = feuilleDe(planche, sens, total);
    if (!feuille) return false;
    const images = new Map<number, Raster>();
    for (const n of [feuille.depuis, feuille.vers]) {
      const r = rasterPret("album", n + 1, cle, largeur);
      if (!r) return false;
      images.set(n, r);
    }
    setTour({ feuille, images, auto });
    return true;
  };

  /** Le tour est allé au bout : la planche change, et la feuille s'efface sur
   *  la même trame — la planche d'arrivée est déjà dans le cache, donc elle
   *  est peinte avant que quoi que ce soit disparaisse. */
  const conclure = (sens: Sens) => {
    onPlanche(sens);
    setTour(null);
  };

  const annuler = () => setTour(null);

  // Le mouvement ne peut commencer qu'une fois la feuille dans le DOM : un
  // effet de mise en page part après le commit de React et avant la peinture,
  // donc la première trame est écrite au bon endroit et rien ne clignote.
  useLayoutEffect(() => {
    if (!tour) return;
    appliquer(0, tour.feuille.sens);
    if (tour.auto) {
      animer(0, 1, tour.feuille.sens, () => conclure(tour.feuille.sens));
    }
    return arreter;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tour]);

  // Ce qu'App tient de la feuille : une seule commande, et la réponse
  // honnête quand elle n'a pas pu la prendre.
  //
  // Une demande pendant qu'une feuille tourne est **avalée**, et c'est
  // délibéré : une flèche maintenue tourne alors une page toutes les quatre
  // cents millisecondes, ce qui est exactement le rythme d'un livre qu'on
  // feuillette. Laisser passer la demande ferait sauter la planche sous une
  // feuille dont les images sont celles d'avant. Mettre les demandes en file
  // ferait tourner une page de trop après qu'on a lâché la touche. Qui
  // parcourt vite a la table lumineuse (⌘3) et la règle du pied, qui sont
  // faites pour ça. À reprendre au ressenti, au bundle.
  useImperativeHandle(
    ref,
    (): Tourneur => ({
      tourner: (sens) => {
        if (sens !== 1 && sens !== -1) return false;
        if (tour) return true;
        return demarrer(sens, true);
      },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [planche, total, cle, largeur, tour],
  );

  // La planche a changé ailleurs qu'au bout de ce tour — Début, Fin, la table
  // lumineuse, une recomposition : la feuille en vol n'a plus de sujet, et la
  // laisser finir poserait un indice calculé depuis un livre qui a bougé.
  useEffect(() => {
    if (!tour) return;
    if (planche === tour.feuille.depuis || planche === tour.feuille.vers) return;
    arreter();
    setTour(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [planche, tour]);

  // ---- le geste ----------------------------------------------------------

  const fractions = (e: React.PointerEvent) => {
    const r = scene.current?.getBoundingClientRect();
    if (!r || r.width <= 0 || r.height <= 0) return null;
    return { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height, r };
  };

  const surPointeurEntre = (e: React.PointerEvent) => {
    if (geste.current) return;
    const f = fractions(e);
    const c = f ? coinTouche(f.x, f.y) : null;
    const possible = c !== null && feuilleDe(planche, c, total) !== null;
    setCoinSurvole(possible ? c : null);
  };

  const surPointeurBas = (e: React.PointerEvent) => {
    // Une feuille en vol garde la main jusqu'au bout, comme au clavier.
    if (geste.current || tour || e.button !== 0) return;
    const f = fractions(e);
    if (!f) return;
    const sens = coinTouche(f.x, f.y);
    if (sens === null || !feuilleDe(planche, sens, total)) return;
    e.preventDefault();
    scene.current?.setPointerCapture(e.pointerId);
    geste.current = {
      sens,
      id: e.pointerId,
      t0: e.timeStamp,
      x0: e.clientX,
      y0: e.clientY,
      // Sans les images, le coin répond quand même : il tourne la page d'un
      // coup au relâchement. Un coin mort serait pris pour une panne.
      anime: demarrer(sens, false),
      vu: { p: 0, t: e.timeStamp },
      vitesse: 0,
    };
  };

  const surPointeurBouge = (e: React.PointerEvent) => {
    const g = geste.current;
    if (!g || g.id !== e.pointerId) return surPointeurEntre(e);
    if (!g.anime) return;
    const r = scene.current?.getBoundingClientRect();
    if (!r) return;
    const p = progresDuPointeur(e.clientX - r.left, r.width, g.sens);
    const dt = e.timeStamp - g.vu.t;
    // Une vitesse prise sur deux trames voisines est du bruit ; huit
    // millisecondes suffisent à la rendre lisible sans la rendre molle.
    if (dt >= 8) {
      g.vitesse = ((p - g.vu.p) / dt) * 1000;
      g.vu = { p, t: e.timeStamp };
    }
    appliquer(p, g.sens);
  };

  const surPointeurHaut = (e: React.PointerEvent) => {
    const g = geste.current;
    if (!g || g.id !== e.pointerId) return;
    geste.current = null;
    setCoinSurvole(null);
    const course = Math.hypot(e.clientX - g.x0, e.clientY - g.y0);
    const clic = estUnClic(course, e.timeStamp - g.t0);
    // Sans feuille montée, le coin n'a rien montré : seul un clic tourne la
    // page. Un glisser qui n'a rien fait bouger ne doit rien décider non plus.
    if (!g.anime) {
      if (clic) onPlanche(g.sens);
      return;
    }
    const verdict = clic ? "termine" : issue(progres.current, g.vitesse);
    if (verdict === "termine") {
      animer(progres.current, 1, g.sens, () => conclure(g.sens));
    } else {
      animer(progres.current, 0, g.sens, annuler);
    }
  };

  const surPointeurPerdu = (e: React.PointerEvent) => {
    const g = geste.current;
    if (!g || g.id !== e.pointerId) return;
    geste.current = null;
    setCoinSurvole(null);
    if (g.anime) animer(progres.current, 0, g.sens, annuler);
  };

  // ---- ce qui est à l'écran ----------------------------------------------

  const hauteur = affiche?.hauteur ?? 0;
  const style = { width: `${largeur}px`, height: `${hauteur}px` };
  const f = tour?.feuille;
  const image = (n: number) => tour?.images.get(n) ?? null;

  return (
    <div
      ref={scene}
      className={"feuilletage" + (coinSurvole !== null ? " au-coin" : "")}
      style={style}
      // Le nom vit sur la scène et pas sur un morceau : au repos il y a un
      // rectangle, au milieu d'un tour il y en a quatre, et l'aperçu fidèle
      // reste une seule chose à annoncer dans les deux cas. Ce qui est peint
      // dedans est du décor, comme partout ailleurs dans ce projet.
      role="img"
      aria-label={t("fidele.pdf.aria")}
      onPointerDown={surPointeurBas}
      onPointerMove={surPointeurBouge}
      onPointerUp={surPointeurHaut}
      onPointerCancel={surPointeurPerdu}
      onPointerLeave={() => !geste.current && setCoinSurvole(null)}
    >
      {/* Au repos, la planche entière, telle que le PDF la porte. Pendant un
          tour, les deux moitiés qui ne bougent pas, prises sur les deux
          planches que la feuille joint. */}
      {f ? (
        <>
          <Demi raster={image(f.fixeGauche.planche)} cote="gauche" ou="gauche" />
          <Demi raster={image(f.fixeDroite.planche)} cote="droite" ou="droite" />
          <div
            ref={ombre}
            className={
              "feuille-ombre " +
              (f.sens === 1 ? "ombre-droite" : "ombre-gauche")
            }
            aria-hidden="true"
          />
          <div
            ref={feuillet}
            className={"feuillet " + (f.sens === 1 ? "vers-avant" : "vers-arriere")}
            aria-hidden="true"
          >
            <div className="face recto">
              <Demi raster={image(f.recto.planche)} cote={f.recto.cote} ou="pleine" />
            </div>
            <div className="face verso">
              <Demi raster={image(f.verso.planche)} cote={f.verso.cote} ou="pleine" />
            </div>
            <div ref={courbure} className="feuille-courbure" />
          </div>
        </>
      ) : (
        <Demi raster={affiche} cote="entier" ou="pleine" />
      )}
      {/* Les deux coins, décor seul : ils disent où le geste vit, ils ne le
          prennent pas. Le test de départ lit `coinTouche`, une seule fois,
          pour que la zone visible et la zone active soient la même chose. */}
      <div
        className={"coin coin-avant" + (coinSurvole === 1 ? " vif" : "")}
        style={{ width: pourcent(COIN.largeur), height: pourcent(COIN.hauteur) }}
        aria-hidden="true"
      />
      <div
        className={"coin coin-arriere" + (coinSurvole === -1 ? " vif" : "")}
        style={{ width: pourcent(COIN.largeur), height: pourcent(COIN.hauteur) }}
        aria-hidden="true"
      />
    </div>
  );
}

type Geste = {
  sens: Sens;
  id: number;
  t0: number;
  x0: number;
  y0: number;
  /** La feuille est montée et suit le doigt. Faux quand les images
   *  manquaient : le coin répond alors d'un seul coup. */
  anime: boolean;
  vu: { p: number; t: number };
  /** Tours par seconde, signée : c'est elle qui décide d'une chiquenaude. */
  vitesse: number;
};

function pourcent(f: number): string {
  return `${(f * 100).toFixed(3)}%`;
}

/**
 * Une moitié de planche — ou la planche entière — peinte depuis le bitmap
 * partagé. Un `drawImage` et rien d'autre : pas de mise à l'échelle CSS, donc
 * pas de page floue, et pas de second dessin du PDF.
 *
 * La peinture se fait avant l'affichage (`useLayoutEffect`) : au bout d'un
 * tour, la planche d'arrivée est à l'écran sur la trame même où la feuille
 * disparaît.
 */
function Demi({
  raster,
  cote,
  ou,
}: {
  raster: Raster | null;
  cote: Cote | "entier";
  /** Où la poser dans la scène : une moitié, ou tout l'espace donné. */
  ou: "gauche" | "droite" | "pleine";
}) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useLayoutEffect(() => {
    const c = canvas.current;
    if (!c || !raster) return;
    const entier = cote === "entier";
    // Le pli tombe au milieu du bitmap, mais un bitmap de largeur impaire n'a
    // pas de milieu entier : découper à `width / 2` échantillonnerait la moitié
    // droite un demi-pixel à côté, et le pli se verrait flou. Les deux moitiés
    // se partagent donc des pixels entiers, quitte à ce que l'une en ait un de
    // plus que l'autre.
    const pli = Math.round(raster.source.width / 2);
    const largeurSource = entier
      ? raster.source.width
      : cote === "droite"
        ? raster.source.width - pli
        : pli;
    const depart = cote === "droite" ? pli : 0;
    c.width = largeurSource;
    c.height = raster.source.height;
    c.style.width = `${entier ? raster.largeur : raster.largeur / 2}px`;
    c.style.height = `${raster.hauteur}px`;
    const ctx = c.getContext("2d");
    ctx?.drawImage(
      raster.source,
      depart,
      0,
      largeurSource,
      raster.source.height,
      0,
      0,
      c.width,
      c.height,
    );
  }, [raster, cote]);

  return <canvas ref={canvas} className={`demi demi-${ou}`} aria-hidden="true" />;
}
