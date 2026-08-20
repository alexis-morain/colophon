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

import { Rect } from "./album";
import { t } from "./i18n";
import { Scene, SceneObject } from "./scene";

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
 *  are painted in. The rank comes from the template, which declared it. */
function enOrdreDeLecture(scene: Scene): { o: SceneObject; depth: number }[] {
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

export function SceneProxies({
  scene,
  mm,
  trim,
  selected,
  onActivate,
  onEchap,
}: {
  scene: Scene;
  mm: number;
  /** The trimmed page inside the media box, millimetres, top-left origin. */
  trim: Rect;
  /** The selected cell, so a photograph can say whether it is chosen. */
  selected?: number | null;
  onActivate: (o: SceneObject, depth: number) => void;
  /** Escape lets go: the spread gets its arrow keys back. */
  onEchap: () => void;
}) {
  const bouge = (el: HTMLElement, sens: 1 | -1) => {
    const voisin =
      sens === 1 ? el.nextElementSibling : el.previousElementSibling;
    if (voisin instanceof HTMLElement) voisin.focus();
  };

  return (
    <div className="scene-proxies" role="group" aria-label={t("scene.objets")}>
      {enOrdreDeLecture(scene).map(({ o, depth }) => {
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
            // The paper deselects on click; a proxy that let its own click
            // through would select and deselect in the same breath.
            onClick={(e) => {
              e.stopPropagation();
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
