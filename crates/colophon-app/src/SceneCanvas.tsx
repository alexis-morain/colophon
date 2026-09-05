// The spread, painted. One element instead of one per object.
//
// **It draws the same picture the DOM renderer draws** — the same type
// inflation, the same legibility floors, the same veil over the cases that
// are not chosen. That is deliberate: wave 2.5 compares two renderers, and a
// comparison between two different pictures would measure the pictures. The
// one place they part is the rhythm of a block of text, which the DOM lays
// out with CSS line boxes and this draws on the scene's own baselines; the
// faithful preview (⇧⌘P) remains the only thing that shows what prints.
//
// The transform is set once so everything below is written in millimetres,
// exactly as the scene carries them. No second coordinate system, no scale
// factor threaded through forty call sites.
//
// It has no state and no gestures. What sits under the pointer is
// `scene.ts::hitTest`, which was written before this existed and is tested
// without a canvas; the handlers live with the rest of the editing in
// `SpreadView`.

import { useEffect, useRef } from "react";
import {
  CAPTION_SIZE_MM,
  PHOTO_CAPTION_SIZE_MM,
  cropWindow,
  Rect,
  SpreadGeometry,
} from "./album";
import { angleEcran, centre, Point, Scene } from "./scene";
import { imageDe, imageRegleeDe, surImage } from "./photos";
import { filtreDe, reglagePose, surReglage } from "./reglages";

/** The screen has always shown small type bigger than it prints: below
 *  these, in CSS pixels, a line stops being readable on a screen. The DOM
 *  renderer's own numbers, so the two pictures agree. A block of text gets
 *  one floor rather than three, because the half-title, the text page and
 *  the colophon are one role here and telling them apart again would undo
 *  what the scene was for. */
const PLANCHER_LEGENDE = 9;
const PLANCHER_TEXTE = 11;
/** Screen type is drawn a third larger than it prints, like the DOM's. */
const GROSSISSEMENT = 1.35;

type Couleurs = {
  paper: string;
  ink: string;
  inkSoft: string;
  accent: string;
  voile: string;
  vide: string;
  police: string;
};

/** The palette, read from the document so the canvas follows the theme the
 *  same way every other surface does: one place declares these colours, and
 *  it is the stylesheet. */
function couleurs(el: HTMLElement): Couleurs {
  const cs = getComputedStyle(el);
  const v = (nom: string) => cs.getPropertyValue(nom).trim();
  return {
    paper: v("--paper"),
    ink: v("--paper-ink"),
    inkSoft: v("--paper-ink-soft"),
    accent: v("--accent"),
    voile: `rgb(${v("--paper-rgb")} / 0.4)`,
    vide: `rgb(${v("--ink-rgb")} / 0.06)`,
    police: v("--font-book") || "sans-serif",
  };
}

export function peindre(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  g: SpreadGeometry,
  mm: number,
  /** Device pixels per CSS pixel. It multiplies both transforms below; a
   *  transform that replaced it instead would paint a quarter of the sheet
   *  on a retina screen and nothing anywhere else. */
  dpr: number,
  c: Couleurs,
  etat: { selected?: number | null; drop?: number | null },
): void {
  ctx.save();
  ctx.setTransform(dpr * mm, 0, 0, dpr * mm, 0, 0);
  ctx.clearRect(0, 0, g.w, g.h);
  ctx.fillStyle = c.paper;
  ctx.fillRect(0, 0, g.w, g.h);

  const texte = (
    s: string,
    x: number,
    baseline: number,
    tailleMm: number,
    plancher: number,
    couleur: string,
    /** L'objet tourne : son angle et le centre qu'il tourne autour, en
     *  millimètres. Une rotation par objet, jamais une par ligne — le flux
     *  PDF pose une matrice par objet lui aussi. */
    rotation?: { angle: number; centre: Point },
  ) => {
    // Type is set in pixels, so the transform has to go for one call: a
    // millimetre-sized font would round to nothing at these scales.
    const px = Math.max(tailleMm * mm * GROSSISSEMENT, plancher);
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (rotation && rotation.angle !== 0) {
      // En pixels CSS, puisque le repère vient d'y passer pour la fonte.
      ctx.translate(rotation.centre.x * mm, rotation.centre.y * mm);
      ctx.rotate((angleEcran(rotation.angle) * Math.PI) / 180);
      ctx.translate(-rotation.centre.x * mm, -rotation.centre.y * mm);
    }
    ctx.font = `${px}px ${c.police}`;
    ctx.textBaseline = "alphabetic";
    ctx.fillStyle = couleur;
    ctx.fillText(s, x * mm, baseline * mm);
    ctx.restore();
  };

  const cases = new Map<number, Rect>();

  for (const o of scene.objects) {
    const role = o.role;
    switch (role.role) {
      case "photo": {
        cases.set(role.cell, o.rect);
        const img = imageDe(role.src);
        if (!img) {
          ctx.fillStyle = c.vide;
          ctx.fillRect(o.rect.x, o.rect.y, o.rect.w, o.rect.h);
          break;
        }
        // The same cover-crop the print performs, read from the same
        // function: a preview that framed differently would be a lie.
        const [sx, sy, sw, sh] = cropWindow(
          o.rect,
          img.naturalWidth,
          img.naturalHeight,
          role.focal,
          role.zoom,
        );
        // The adjustment, where the pixels resolve. `ctx.filter` when the
        // engine has it (WebView2 yes; WKWebView from Safari 18 — detect
        // the property, never the agent), the draft riding along like on
        // the DOM. Without it, the fallback is a bitmap pre-adjusted by the
        // LUT port — committed réglage only, so a drag costs nothing and
        // the case follows at release. The fallback is never « no
        // adjustment »: a canvas that dropped the réglage would be a
        // renderer that lies.
        const filtre = typeof ctx.filter === "string";
        const regle = filtre ? undefined : reglagePose(role.src);
        const source =
          regle !== undefined ? imageRegleeDe(role.src, regle) ?? img : img;
        ctx.save();
        ctx.beginPath();
        ctx.rect(o.rect.x, o.rect.y, o.rect.w, o.rect.h);
        ctx.clip();
        if (filtre) ctx.filter = filtreDe(role.src) ?? "none";
        ctx.drawImage(source, sx, sy, sw, sh, o.rect.x, o.rect.y, o.rect.w, o.rect.h);
        ctx.restore();
        break;
      }
      case "photo_caption":
        texte(
          role.text,
          role.at.x,
          role.at.y,
          PHOTO_CAPTION_SIZE_MM,
          PLANCHER_LEGENDE,
          c.inkSoft,
        );
        break;
      // No floor: the DOM renderer gives the chapter caption none either,
      // and a title that shrinks with the window is the one piece of type
      // whose size a reader is judging.
      case "chapter_caption":
        texte(role.text, role.at.x, role.at.y, CAPTION_SIZE_MM, 0, c.ink);
        break;
      case "text":
        for (const l of role.lines) {
          texte(
            l.text,
            role.at.x,
            role.at.y + l.dyMm,
            l.sizeMm,
            PLANCHER_TEXTE,
            c.ink,
          );
        }
        break;
      // Un objet libre : les mêmes trois nombres que le DOM et que
      // l'émetteur, et la rotation posée une fois pour l'objet.
      case "free_text": {
        const rotation = { angle: o.angle, centre: centre(o.rect) };
        for (const l of role.lines) {
          texte(
            l.text,
            role.at.x + l.dxMm,
            role.at.y + l.dyMm,
            l.sizeMm,
            PLANCHER_TEXTE,
            c.ink,
            rotation,
          );
        }
        break;
      }
    }
  }

  // The selection reads twice, like the stylesheet says: a light veil on
  // every case that is not the chosen one, a border on the one that is.
  if (etat.selected !== null && etat.selected !== undefined) {
    ctx.fillStyle = c.voile;
    for (const [cell, r] of cases) {
      if (cell !== etat.selected) ctx.fillRect(r.x, r.y, r.w, r.h);
    }
    const choisie = cases.get(etat.selected);
    if (choisie) cadre(ctx, choisie, c.accent, mm, false);
  }
  if (etat.drop !== null && etat.drop !== undefined) {
    const cible = cases.get(etat.drop);
    if (cible) cadre(ctx, cible, c.accent, mm, true);
  }

  ctx.restore();
}

/** A two-pixel border inside a cell, so the page geometry never shifts. */
function cadre(
  ctx: CanvasRenderingContext2D,
  r: Rect,
  couleur: string,
  mm: number,
  pointille: boolean,
): void {
  const e = 2 / mm;
  ctx.save();
  ctx.strokeStyle = couleur;
  ctx.lineWidth = e;
  if (pointille) ctx.setLineDash([6 / mm, 4 / mm]);
  ctx.strokeRect(r.x + e / 2, r.y + e / 2, r.w - e, r.h - e);
  ctx.restore();
}

export function SceneCanvas({
  scene,
  geom,
  mm,
  selected,
  drop,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onDoubleClick,
  onDragOver,
  onDragLeave,
  onDrop,
  canvasRef,
}: {
  scene: Scene;
  geom: SpreadGeometry;
  mm: number;
  selected?: number | null;
  /** The cell a drop would land in, while a drag hovers it. */
  drop?: number | null;
  onPointerDown?: (e: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerMove?: (e: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerUp?: (e: React.PointerEvent<HTMLCanvasElement>) => void;
  onDoubleClick?: (e: React.MouseEvent<HTMLCanvasElement>) => void;
  onDragOver?: (e: React.DragEvent<HTMLCanvasElement>) => void;
  onDragLeave?: () => void;
  onDrop?: (e: React.DragEvent<HTMLCanvasElement>) => void;
  canvasRef: React.RefObject<HTMLCanvasElement | null>;
}) {
  const dernier = useRef<() => void>(() => {});

  useEffect(() => {
    const el = canvasRef.current;
    if (!el) return;
    const dpr = window.devicePixelRatio || 1;
    const dessiner = () => {
      const ctx = el.getContext("2d");
      if (!ctx) return;
      const w = Math.round(geom.w * mm * dpr);
      const h = Math.round(geom.h * mm * dpr);
      if (el.width !== w || el.height !== h) {
        el.width = w;
        el.height = h;
      }
      peindre(ctx, scene, geom, mm, dpr, couleurs(el), { selected, drop });
    };
    dernier.current = dessiner;
    dessiner();
    // A thumbnail that lands after the first paint repaints the page: the
    // canvas has no `<img>` to wait for on its behalf. Same deal for an
    // adjustment — a réglage committed without a repaint is a picture that
    // lies until the next nudge.
    const offImage = surImage(() => dernier.current());
    const offReglage = surReglage(() => dernier.current());
    return () => {
      offImage();
      offReglage();
    };
  });

  return (
    <canvas
      ref={canvasRef}
      className="spread-canvas"
      // Un canvas n'a pas d'éléments : ce qu'il peint est nommé par la
      // couche de proxies, jamais par lui. Le déclarer ferme la porte au
      // jour où un navigateur exposerait la surface elle-même.
      aria-hidden="true"
      style={{ width: `${geom.w * mm}px`, height: `${geom.h * mm}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      // Le papier désélectionne au clic, et le navigateur fabrique un clic
      // à la fin de tout geste, glisser compris. Les cases du rendu DOM
      // avalent le leur ; sans celui-ci, sélectionner puis désélectionner
      // tenaient dans le même souffle. La sélection au canvas se décide
      // dans le pointerup, jamais ici — cliquer du vide désélectionne déjà.
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={onDoubleClick}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    />
  );
}
