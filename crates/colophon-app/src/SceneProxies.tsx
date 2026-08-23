// The keyboard's hold on a spread: one focusable box per object of the
// scene, in reading order.
//
// **This is a function of the scene, and of nothing else.** It reads the
// rectangles, the reading ranks and the role codes — never a rendered
// sentence, because a string born in the engine stays French on an English
// screen. Which is why it does not care what draws underneath it: the DOM
// renderer today, a canvas tomorrow, and not a line of this changes. A
// canvas has no elements at all, so without this layer the keyboard would
// have nothing to hold; laying it down first gives the port a target it is
// not allowed to regress.
//
// It is not a preservation either. Before it, a spread carried exactly one
// `aria-hidden` — on the fold — and a case could only be selected with the
// mouse.
//
// **Invisible to the pointer.** The layer is `pointer-events: none`, so
// every click, drag and wheel still reaches the render below, untouched.
// Focus is not hit testing: Tab reaches these boxes all the same.
//
// **And it keeps what it holds.** Turning a spread destroys these boxes and
// builds new ones, which drops the focus on the document body — silently
// sending whoever was reading back to the top of the page. Here the
// keyboard is put back at the same reading rank on the new spread, and a
// field opened from a box gives the focus back to that box when it closes.

import { useEffect, useRef } from "react";
import { Rect } from "./album";
import { t } from "./i18n";
import { Scene, SceneObject } from "./scene";

// ---- où le clavier se tient, et pourquoi ça vit hors du composant --------
//
// La planche qui tourne porte une clé de rendu (`key={index}`, c'est elle qui
// rejoue l'animation de page) : la couche entière est donc démontée et
// rebâtie à chaque tour, refs et états compris. Une mémoire de composant
// serait détruite précisément à l'instant où elle sert. Elle vit donc à côté,
// où le démontage ne l'atteint pas — une seule planche est à l'écran à la
// fois, comme il n'y a qu'une langue et qu'un rendu, et pour la même raison
// un module suffit.

/** Le rang de lecture que le clavier tient, tant qu'il le tient. */
let rangClavier: number | null = null;
/** Consigne posée par la couche qui s'en va, lue par celle qui arrive. */
let aRendre: number | "fin" | null = null;
/** La planche que la dernière couche montée tenait : c'est son changement
 *  qui justifie de rendre le focus, pas un simple remontage de vue. */
let plancheTenue: number | null = null;

/** Le pointeur reprend la main : le clavier n'a plus de place gardée. Sans
 *  ça, un clic sur le papier laisserait la mémoire debout et la planche
 *  suivante volerait le focus à quelqu'un qui ne l'avait pas demandé. */
function surPointeur() {
  rangClavier = null;
}

/**
 * What to call one object, out loud. Built from the role code and its
 * parameters, so both languages say it their own way and neither inherits
 * the other's word order.
 */
export function nomDObjet(o: SceneObject, scene: Scene): string {
  const role = o.role;
  switch (role.role) {
    case "photo": {
      const total = scene.objects.filter((x) => x.role.role === "photo").length;
      // The file name is the only handle that tells two photographs apart
      // out loud. Its folder is not: a spoken path is noise, and the rest of
      // the application never says one either.
      const fichier = role.src.split("/").pop() ?? role.src;
      return t("scene.photo", { n: role.cell + 1, total, fichier });
    }
    case "photo_caption":
      return t("scene.legende", { n: role.cell + 1, texte: role.text });
    case "chapter_caption":
      return role.text
        ? t("scene.chapitre", { texte: role.text })
        : t("scene.chapitre.vide");
    case "text":
      return t("scene.texte", { texte: role.lines[0]?.text ?? "" });
  }
}

/** Objects in the order a person reads them, which is not the order they
 *  are painted in. The rank comes from the template, which declared it.
 *
 *  Exportée pour être épinglée : c'est l'ordre où l'application prononce une
 *  planche, et il ne se lit nulle part ailleurs. */
export function enOrdreDeLecture(scene: Scene): { o: SceneObject; depth: number }[] {
  return scene.objects
    .map((o, depth) => ({ o, depth }))
    .sort((a, b) => a.o.reading - b.o.reading || a.depth - b.depth);
}

/**
 * The part of an object that lands on the trimmed page.
 *
 * A full-bleed photograph runs past the guillotine on every side, and the
 * sheet on screen is clipped there: a focus ring drawn on the object's own
 * edge would be cut away with the bleed and the reader would see nothing at
 * all. The proxy is a target, not a measurement — the rectangle stays whole
 * everywhere it is measured, and only the box the keyboard lands on is
 * brought back inside the page.
 */
function auRognage(r: Rect, trim: Rect): Rect {
  const x = Math.max(r.x, trim.x);
  const y = Math.max(r.y, trim.y);
  return {
    x,
    y,
    w: Math.max(Math.min(r.x + r.w, trim.x + trim.w) - x, 0),
    h: Math.max(Math.min(r.y + r.h, trim.y + trim.h) - y, 0),
  };
}

/** Un objet dont l'activation ouvre un vrai champ de saisie, donc dont le
 *  focus part ailleurs et doit revenir. */
const ouvreUnChamp = (o: SceneObject) =>
  o.role.role === "chapter_caption" || o.role.role === "text";

export function SceneProxies({
  scene,
  mm,
  trim,
  selected,
  planche,
  edition,
  onActivate,
  onEchap,
  onPlanche,
}: {
  scene: Scene;
  mm: number;
  /** The trimmed page inside the media box, millimetres, top-left origin. */
  trim: Rect;
  /** The selected cell, so a photograph can say whether it is chosen. */
  selected?: number | null;
  /** Quelle planche ces boîtes tiennent : c'est son changement qui les
   *  détruit, et donc lui qui commande de rendre le focus. */
  planche: number;
  /** Vrai tant qu'un champ de saisie est ouvert sur la planche. */
  edition: boolean;
  onActivate: (o: SceneObject, depth: number) => void;
  /** Escape lets go: the spread gets its arrow keys back. */
  onEchap: () => void;
  /** Au bout de l'ordre de lecture, la lecture continue à la planche d'à
   *  côté. Rend vrai si la planche a effectivement tourné. */
  onPlanche?: (sens: 1 | -1) => boolean;
}) {
  const conteneur = useRef<HTMLDivElement>(null);
  /** Le champ ouvert l'a été depuis une boîte, donc le focus lui revient. */
  const champDepuisIci = useRef(false);

  /** Poser le clavier sur une boîte, sans jamais le voler à qui le tient. */
  const rendreLeClavier = (cible: number | "fin") => {
    const el = conteneur.current;
    const n = el?.children.length ?? 0;
    if (!el || n === 0) return;
    const actif = document.activeElement;
    if (actif && actif !== document.body && !el.contains(actif)) return;
    const i = cible === "fin" ? n - 1 : Math.min(cible, n - 1);
    const boite = el.children[i];
    if (boite instanceof HTMLElement) boite.focus();
  };

  useEffect(() => {
    window.addEventListener("pointerdown", surPointeur, true);
    return () => window.removeEventListener("pointerdown", surPointeur, true);
  }, []);

  // La couche vient de naître. Si la précédente tenait une autre planche et
  // que le clavier s'y trouvait, il reprend au même rang de lecture — borné
  // au nombre d'objets, parce que feuilleter en regardant la même case est
  // le geste réel. La consigne d'une lecture continue, elle, dit où reprendre.
  useEffect(() => {
    const tourne = plancheTenue !== null && plancheTenue !== planche;
    const cible = tourne ? (aRendre ?? rangClavier) : null;
    aRendre = null;
    plancheTenue = planche;
    if (cible !== null) rendreLeClavier(cible);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [planche]);

  // Un champ ouvert depuis une boîte vient de se refermer : le focus est
  // retombé sur le document, et personne ne le ramène. Ici la couche n'a pas
  // été démontée — seule la planche qui tourne le fait.
  useEffect(() => {
    if (edition || !champDepuisIci.current) return;
    champDepuisIci.current = false;
    if (rangClavier !== null) rendreLeClavier(rangClavier);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edition]);

  const bouge = (el: HTMLElement, sens: 1 | -1) => {
    const voisin =
      sens === 1 ? el.nextElementSibling : el.previousElementSibling;
    if (voisin instanceof HTMLElement) {
      voisin.focus();
      return;
    }
    // Au bout de l'ordre de lecture, la lecture continue sur la planche
    // d'à côté, et reprend par son premier objet : c'est un livre. Une
    // planche tournée d'ailleurs — le menu — garde le rang, elle.
    if (!onPlanche) return;
    aRendre = sens === 1 ? 0 : "fin";
    if (!onPlanche(sens)) aRendre = null;
  };

  return (
    <div
      ref={conteneur}
      className="scene-proxies"
      role="group"
      aria-label={t("scene.objets")}
    >
      {enOrdreDeLecture(scene).map(({ o, depth }, i) => {
        const box = auRognage(o.rect, trim);
        return (
          <button
            key={depth}
            type="button"
            className="scene-proxy"
            style={{
              left: `${box.x * mm}px`,
              top: `${box.y * mm}px`,
              width: `${box.w * mm}px`,
              height: `${box.h * mm}px`,
            }}
            aria-label={nomDObjet(o, scene)}
            aria-pressed={
              o.role.role === "photo" ? selected === o.role.cell : undefined
            }
            onFocus={() => (rangClavier = i)}
            // Tab s'en va ailleurs : la place gardée n'a plus lieu d'être.
            // Un `relatedTarget` nul, lui, ne dit rien — c'est aussi bien la
            // boîte qu'on détruit sous le focus que le corps du document.
            // Et le champ qu'on vient d'ouvrir depuis cette boîte n'est pas
            // un ailleurs : il rendra le focus en se refermant, donc le rang
            // doit lui survivre — sans ça la restauration n'a rien à rendre.
            onBlur={(e) => {
              const vers = e.relatedTarget;
              if (
                vers instanceof Node &&
                !conteneur.current?.contains(vers) &&
                !champDepuisIci.current
              ) {
                rangClavier = null;
              }
            }}
            // The paper deselects on click; a proxy that let its own click
            // through would select and deselect in the same breath.
            onClick={(e) => {
              e.stopPropagation();
              if (ouvreUnChamp(o)) champDepuisIci.current = true;
              onActivate(o, depth);
            }}
            onKeyDown={(e) => {
              const el = e.currentTarget;
              switch (e.key) {
                // While a proxy holds the focus the arrows walk the page
                // rather than turn it — App yields the keyboard to a focused
                // button already, so this is a dead key otherwise.
                case "ArrowRight":
                case "ArrowDown":
                  e.preventDefault();
                  bouge(el, 1);
                  break;
                case "ArrowLeft":
                case "ArrowUp":
                  e.preventDefault();
                  bouge(el, -1);
                  break;
                case "Home": {
                  e.preventDefault();
                  const first = el.parentElement?.firstElementChild;
                  if (first instanceof HTMLElement) first.focus();
                  break;
                }
                case "End": {
                  e.preventDefault();
                  const last = el.parentElement?.lastElementChild;
                  if (last instanceof HTMLElement) last.focus();
                  break;
                }
                case "Escape":
                  e.preventDefault();
                  rangClavier = null;
                  el.blur();
                  onEchap();
                  break;
              }
            }}
          />
        );
      })}
    </div>
  );
}
