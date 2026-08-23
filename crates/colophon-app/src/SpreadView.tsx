// One spread, rendered at the trimmed size the reader will hold. The media
// box (bleed included) sits behind, offset so the bleed falls outside the
// visible page, exactly like a trimmed print. When the edit callbacks are
// given, photos become selectable and draggable; the view stays a pure
// reader without them.
//
// **What it draws, it does not decide.** The spread is turned into a scene
// (`scene.ts`) — objects in paint order, each with its rectangle, its
// reading rank and its role — by the same derivation the engine runs before
// writing the PDF. This view walks that list. It no longer rebuilds a
// rectangle, and it no longer knows that `garde`, `texte` and `colophon` are
// special: it knows there is a block of text, and which shape of block the
// screen gives it.
//
// The two things the scene does not hold are the two editor affordances for
// objects that do not exist yet: the ghost of an untitled chapter, and the
// invitation on a blank text page. Both are marked as such below.
//
// The selected photo is a crop editor: dragging inside the case moves the
// framing (⌥ refines), the wheel zooms past the fill, a double-click
// recentres on the detected focal. Gestures work on a local draft and land
// on the undo stack once, at the end of the gesture.

import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Album,
  CAPTION_SIZE_MM,
  PHOTO_CAPTION_SIZE_MM,
  Rect,
  Slot,
  Spread,
  TEXT_LEADING_MM,
  TEXT_SIZE_MM,
  MIN_EFFECTIVE_PPI,
  ZOOM_MAX,
  ZOOM_MIN,
  captionAnchor,
  spreadGeometry,
  slidingRoom,
  textAnchor,
  COLOPHON_TEMPLATE,
  COLOPHON_SIZE_MM,
  COLOPHON_LEADING_MM,
  gardePlace,
  GARDE_TEMPLATE,
} from "./album";
import { captionSuggestion, detectedFocal } from "./bridge";
import { SceneProxies } from "./SceneProxies";
import { fontLoaded, measureMm } from "./font";
import { badgesDe, imageDe, ROOM_EPSILON, surImage } from "./photos";
import { useRendu } from "./rendu";
import { SceneCanvas } from "./SceneCanvas";
import { t } from "./i18n";
import { jusquAuRendu } from "./mesure";
import {
  avecRecadrage,
  hitTest,
  Point,
  Role,
  SceneObject,
  sceneOf,
} from "./scene";
import { cachedThumb, loadThumb } from "./thumbs";

/** A crop being adjusted: values shown before they land on the undo stack. */
type CropDraft = { slot: number; focal: [number, number]; zoom: number };

export function SpreadView({
  album,
  spread,
  planche,
  selected,
  onSelect,
  onSwap,
  onPlace,
  onCrop,
  onCaption,
  onSpreadCaption,
  proposition,
  onText,
  onOverflow,
  onSansMarge,
  onPlanche,
}: {
  album: Album;
  spread: Spread;
  /** Le rang de cette planche dans le livre : la couche d'accessibilité en a
   *  besoin pour savoir qu'elle vient d'être rebâtie, et rendre le clavier. */
  planche: number;
  selected?: number | null;
  onSelect?: (slot: number | null) => void;
  onSwap?: (a: number, b: number) => void;
  /** A drawer photo lands in a case. */
  onPlace?: (slot: number, photo: Slot) => void;
  /** A crop gesture ended: commit focal + zoom for one slot. */
  onCrop?: (slot: number, focal: [number, number], zoom: number) => void;
  /** The caption of one photo changed (the popover under the case). */
  onCaption?: (slot: number, text: string) => void;
  /** The chapter caption was renamed in place. */
  onSpreadCaption?: (caption: string) => void;
  /** The caption proposed while the field is empty: grey in place, Tab
   *  accepts (held by App), any other gesture ignores it. */
  proposition?: string | null;
  /** The free text of a `texte` spread changed. */
  onText?: (text: string) => void;
  /** Some text overflows its room on this spread (signalled, never cut). */
  onOverflow?: (message: string | null) => void;
  /** A crop drag found nothing to slide: the photo fills its cell exactly. */
  onSansMarge?: () => void;
  /** Tourner la planche depuis la couche d'objets, au bout de l'ordre de
   *  lecture. Rend vrai si elle a tourné. */
  onPlanche?: (sens: 1 | -1) => boolean;
}) {
  const paper = useRef<HTMLDivElement>(null);
  const [mm, setMm] = useState(1);
  const [draft, setDraft] = useState<CropDraft | null>(null);
  const [editingCaption, setEditingCaption] = useState(false);
  // The text block's editing state lives here rather than inside the block,
  // because the keyboard opens it from the proxy layer and the mouse opens
  // it from the block itself: one state, two ways in.
  const [editingText, setEditingText] = useState(false);

  const trimW = album.trim_mm.w * 2;
  const geom = spreadGeometry(album);

  // One millimetre in pixels: every geometry below is then written in mm.
  useLayoutEffect(() => {
    const el = paper.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setMm(entry.contentRect.width / trimW);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [trimW]);

  // The draft belongs to one selection on one spread.
  useEffect(() => setDraft(null), [spread, selected]);
  useEffect(() => setEditingCaption(false), [spread]);
  useEffect(() => setEditingText(false), [spread]);

  // Text is only measured in the embedded face: once it lands (local file,
  // milliseconds), render again so every ink rectangle is remeasured. The
  // flag itself is never read — the re-render is the whole point.
  const [, setFontReady] = useState(false);
  useEffect(() => {
    let alive = true;
    fontLoaded().then(() => alive && setFontReady(true));
    return () => {
      alive = false;
    };
  }, []);

  // What this spread holds, derived exactly as the engine derives it before
  // writing the PDF. Rebuilt on every render rather than memoised: it is a
  // handful of objects, and a stale scene would draw yesterday's page.
  //
  // A gesture in flight is substituted into it rather than handed to the
  // renderer on the side: a draft is not another scene, it is these objects
  // with one framing not yet written down — and neither renderer has to
  // learn what a crop draft is.
  const pose = sceneOf(spread, geom, measureMm);
  const scene = draft
    ? avecRecadrage(pose, draft.slot, draft.focal, draft.zoom)
    : pose;
  const cellRects = new Map<number, Rect>();
  scene.objects.forEach((o) => {
    if (o.role.role === "photo") cellRects.set(o.role.cell, o.rect);
  });

  // Photo captions wider than their slot, and text lines wider than the
  // page: named to the reader, never cut.
  useEffect(() => {
    if (!onOverflow) return;
    const problems: string[] = [];
    for (const o of scene.objects) {
      if (o.role.role !== "photo_caption") continue;
      const cell = cellRects.get(o.role.cell);
      if (!cell) continue;
      // Below the trimmed page (full-bleed slots): the caption would print
      // in the bleed and be cut off entirely. Its baseline says so.
      if (o.role.at.y > geom.h - 4) {
        problems.push(t("deborde.legende.horspage", { i: o.role.cell + 1 }));
        continue;
      }
      // The object's own ink is the measurement: nothing is measured twice.
      if (o.rect.w > cell.w) {
        problems.push(
          t("deborde.legende.longue", {
            i: o.role.cell + 1,
            mm: Math.ceil(o.rect.w - cell.w),
          }),
        );
      }
    }
    const bloc = scene.objects.find((o) => o.role.role === "text");
    const lignes = bloc?.role.role === "text" ? bloc.role.lines : [];
    if (spread.template === "texte") {
      const room = geom.w / 2 - geom.margin - geom.gutter / 2;
      const over = lignes.filter((l) => measureMm(l.text, l.sizeMm) > room).length;
      if (over > 0) {
        problems.push(
          over > 1 ? t("deborde.lignes", { n: over }) : t("deborde.ligne.une"),
        );
      }
    }
    // The half-title fits its title by shrinking it, and builds its town
    // line to the page: nothing here overflows unless album.json was
    // repaired by hand, which is precisely when saying so is worth it.
    if (spread.template === GARDE_TEMPLATE) {
      const room = gardePlace(geom);
      if (lignes.some((l) => measureMm(l.text, l.sizeMm) > room + 0.01)) {
        problems.push(t("deborde.garde"));
      }
    }
    onOverflow(problems[0] ?? null);
  });

  // The caption popover anchors under the selected case, in viewport
  // coordinates (position: fixed): it may hang below the sheet without
  // being clipped by the paper's overflow.
  const hasSelection = selected !== null && selected !== undefined;
  const selectedSlot = hasSelection ? (spread.slots[selected] ?? null) : null;
  const selectedRect = hasSelection ? (cellRects.get(selected) ?? null) : null;
  const paperBox = paper.current?.getBoundingClientRect() ?? null;

  /**
   * The chapter caption, whether it exists yet or not. Given an object's
   * text, it draws it where the scene put it; given null, it draws the ghost
   * that invites a title — an editor affordance for an object the spread
   * does not carry, which is why it is the one anchor this view still asks
   * the geometry for. An album.json repaired by hand can also hold an empty
   * caption: it wears the ghost and shows nothing, exactly as before.
   */
  const chapitre = (text: string | null, at: Point) => {
    // L'invitation à nommer un chapitre tient la place d'un objet que la
    // planche ne porte pas : aucun proxy ne la nomme, donc elle est sa
    // propre porte d'entrée — à la souris comme avant, au clavier
    // désormais.
    const invitation = text === null && !!onSpreadCaption;
    return editingCaption && onSpreadCaption ? (
      <input
        className="caption caption-input"
        style={{
          left: `${at.x * mm}px`,
          top: `${at.y * mm}px`,
          fontSize: `${Math.max(CAPTION_SIZE_MM * mm * 1.35, 13)}px`,
        }}
        defaultValue={text ?? ""}
        placeholder={proposition ?? t("planche.chapitre.placeholder")}
        autoFocus
        onFocus={(e) => e.currentTarget.select()}
        onClick={(e) => e.stopPropagation()}
        onBlur={(e) => {
          setEditingCaption(false);
          onSpreadCaption(e.currentTarget.value);
        }}
        onKeyDown={(e) => {
          e.stopPropagation();
          // Sans preventDefault, l'action par défaut d'Entrée clique ce qui
          // tient le focus quand elle s'exécute — or la fermeture rend le
          // focus à la boîte d'origine, et le champ validé se rouvrait.
          if (e.key === "Enter") {
            e.preventDefault();
            e.currentTarget.blur();
          }
          // Tab takes the grey proposal, in the field like outside it.
          if (e.key === "Tab" && proposition && e.currentTarget.value === "") {
            e.preventDefault();
            e.currentTarget.value = proposition;
            e.currentTarget.blur();
          }
          if (e.key === "Escape") {
            e.currentTarget.value = text ?? "";
            e.currentTarget.blur();
          }
        }}
      />
    ) : (
      <span
        className={
          "caption" +
          (onSpreadCaption ? " editable" : "") +
          (text ? "" : " ghost")
        }
        style={{
          left: `${at.x * mm}px`,
          top: `${at.y * mm}px`,
          fontSize: `${CAPTION_SIZE_MM * mm * 1.35}px`,
        }}
        // Ce qu'un rendu peint est du décor : la couche de proxies nomme
        // déjà cet objet, et un nom donné deux fois est un nom lu deux
        // fois. L'invitation fait exception — elle ne double aucun proxy.
        aria-hidden={invitation ? undefined : true}
        role={invitation ? "button" : undefined}
        tabIndex={invitation ? 0 : undefined}
        aria-label={invitation ? t("planche.chapitre.nommer") : undefined}
        onKeyDown={
          invitation
            ? (e) => {
                if (e.key !== "Enter" && e.key !== " ") return;
                e.preventDefault();
                e.stopPropagation();
                setEditingCaption(true);
              }
            : undefined
        }
        title={
          !text && proposition
            ? t("planche.proposition.titre")
            : onSpreadCaption
              ? t("planche.chapitre.renommer")
              : undefined
        }
        onClick={
          onSpreadCaption &&
          ((e) => {
            e.stopPropagation();
            setEditingCaption(true);
          })
        }
      >
        {text ?? proposition ?? t("planche.chapitre.ghost")}
      </span>
    );
  };

  const aChapitre = scene.objects.some((o) => o.role.role === "chapter_caption");
  const aTexte = scene.objects.some((o) => o.role.role === "text");

  /**
   * What is being typed into, whichever renderer is drawing.
   *
   * Renaming a chapter and writing a text page stay **real fields** — a
   * caret, a selection, an undo stack, a spellchecker, an input method:
   * everything a text box owes the person using it, and none of which a
   * canvas can be made to fake convincingly. So the canvas simply stops
   * painting the object the field is showing, for as long as the field is
   * open, and the field sits over the hole.
   */
  const enEdition = (o: SceneObject) =>
    (o.role.role === "chapter_caption" && editingCaption && !!onSpreadCaption) ||
    (o.role.role === "text" && editingText && !!onText);

  /**
   * What Enter does on a focused object — the keyboard's half of what a
   * click already does. A caption leads to its own photograph, because the
   * caption editor is the popover under the case: the reader who reaches a
   * caption wants to write one.
   */
  const activer = (o: SceneObject) => {
    switch (o.role.role) {
      case "photo":
        onSelect?.(selected === o.role.cell ? null : o.role.cell);
        break;
      case "photo_caption":
        onSelect?.(o.role.cell);
        break;
      case "chapter_caption":
        if (onSpreadCaption) setEditingCaption(true);
        break;
      case "text":
        if (onText && spread.template === "texte") setEditingText(true);
        break;
    }
  };

  // ---- the canvas renderer, behind its switch --------------------------
  // Both renderers eat the same scene, so nothing below is a second opinion
  // about what a spread holds: only about how it reaches the screen. The
  // default stays `dom` until 2.5 has measured the two.
  const modeCanvas = useRendu() === "canvas";
  const canvas = useRef<HTMLCanvasElement>(null);
  const [drop, setDrop] = useState<number | null>(null);
  const geste = useRef<{
    type: "crop" | "swap" | "vide";
    cell: number;
    x: number;
    y: number;
    focal: [number, number];
    moved: boolean;
    signale: boolean;
    at: number | null;
  } | null>(null);
  // A canvas has no `<img>` to wait on its behalf: a thumbnail landing has
  // to repaint the badges too, not only the picture.
  const [, setArrivee] = useState(0);
  useEffect(
    () => (modeCanvas ? surImage(() => setArrivee((n) => n + 1)) : undefined),
    [modeCanvas],
  );

  /** Pointer coordinates in the scene's own frame: millimetres, top-left of
   *  the media box, which is exactly what the canvas covers. */
  const enMm = (e: { clientX: number; clientY: number }) => {
    const r = canvas.current?.getBoundingClientRect();
    if (!r) return { x: -1, y: -1 };
    return { x: (e.clientX - r.left) / mm, y: (e.clientY - r.top) / mm };
  };

  /** The cell under a point, through the scene: the hit test reads the
   *  paint order backwards, so a caption over a photograph answers for the
   *  photograph it names — which is the case a gesture is about. */
  const caseSous = (x: number, y: number): number | null => {
    const at = hitTest(scene, x, y);
    if (at === null) return null;
    const role = scene.objects[at].role;
    return role.role === "photo" || role.role === "photo_caption"
      ? role.cell
      : null;
  };

  const surPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    const p = enMm(e);
    const at = hitTest(scene, p.x, p.y);
    const cell = caseSous(p.x, p.y);
    e.currentTarget.setPointerCapture(e.pointerId);
    const commun = { x: e.clientX, y: e.clientY, moved: false, signale: false, at };
    if (cell !== null && cell === selected && onCrop) {
      const slot = spread.slots[cell];
      const f = (draft?.slot === cell ? draft.focal : slot?.focal) ?? [0.5, 0.42];
      geste.current = { type: "crop", cell, focal: [f[0], f[1]], ...commun };
    } else if (cell !== null) {
      geste.current = { type: "swap", cell, focal: [0, 0], ...commun };
    } else {
      geste.current = { type: "vide", cell: -1, focal: [0, 0], ...commun };
    }
  };

  const surPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const g = geste.current;
    if (!g) return;
    const dx0 = e.clientX - g.x;
    const dy0 = e.clientY - g.y;
    if (!g.moved && Math.abs(dx0) + Math.abs(dy0) < 3) return;
    g.moved = true;
    if (g.type === "swap") {
      const p = enMm(e);
      const cible = caseSous(p.x, p.y);
      setDrop(cible !== null && cible !== g.cell ? cible : null);
      return;
    }
    if (g.type !== "crop") return;
    const slot = spread.slots[g.cell];
    const img = slot ? imageDe(slot.src) : null;
    const r = cellRects.get(g.cell);
    if (!img?.naturalWidth || !r) return;
    const zoom = draft?.slot === g.cell ? draft.zoom : (slot.zoom ?? 1);
    const { x: spanX, y: spanY } = slidingRoom(
      { w: r.w * mm, h: r.h * mm },
      img.naturalWidth,
      img.naturalHeight,
      zoom,
    );
    // ⌥ affine : the same fifth of a pixel per pixel as the DOM renderer.
    const fine = e.altKey ? 0.2 : 1;
    const dx = dx0 * fine;
    const dy = dy0 * fine;
    if (spanX <= ROOM_EPSILON && spanY <= ROOM_EPSILON) {
      if (!g.signale) {
        g.signale = true;
        onSansMarge?.();
      }
      return;
    }
    const fx = spanX > 0.5 ? g.focal[0] - dx / spanX : g.focal[0];
    const fy = spanY > 0.5 ? g.focal[1] - dy / spanY : g.focal[1];
    const fin = jusquAuRendu("recadrage.trame");
    setDraft({
      slot: g.cell,
      focal: [Math.min(1, Math.max(0, fx)), Math.min(1, Math.max(0, fy))],
      zoom,
    });
    fin();
  };

  const surPointerUp = () => {
    const g = geste.current;
    geste.current = null;
    const cible = drop;
    setDrop(null);
    if (!g) return;
    if (!g.moved) {
      // A click that moved nothing is a click: the same thing the proxy
      // layer does with Enter, and the same thing a `<div>` used to do.
      if (g.at === null) onSelect?.(null);
      else activer(scene.objects[g.at]);
      return;
    }
    if (g.type === "crop" && draft?.slot === g.cell && onCrop) {
      const d = draft;
      setDraft(null);
      onCrop(g.cell, d.focal, d.zoom);
    }
    if (g.type === "swap" && cible !== null && onSwap) onSwap(g.cell, cible);
  };

  const surDoubleClic = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    const p = enMm(e);
    const cell = caseSous(p.x, p.y);
    if (cell === null || cell !== selected || !onCrop) return;
    const f = await detectedFocal(spread.slots[cell]?.src ?? "").catch(
      () => [0.5, 0.42] as [number, number],
    );
    onCrop(cell, [f[0], f[1]], spread.slots[cell]?.zoom ?? 1);
  };

  // The wheel needs a listener the browser cannot treat as passive, or the
  // page scrolls under the zoom. One burst commits once, when it stops.
  const molette = useRef<{ focal: [number, number]; zoom: number } | null>(null);
  const moletteTimer = useRef<number | undefined>(undefined);
  useEffect(() => {
    const el = canvas.current;
    if (!el || !modeCanvas || selected === null || selected === undefined) return;
    if (!onCrop) return;
    const cell = selected;
    const surMolette = (e: WheelEvent) => {
      const r = el.getBoundingClientRect();
      if (caseSous((e.clientX - r.left) / mm, (e.clientY - r.top) / mm) !== cell) {
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      const slot = spread.slots[cell];
      const cur =
        molette.current ?? {
          focal: (draft?.slot === cell ? draft.focal : slot?.focal) ?? [0.5, 0.42],
          zoom: draft?.slot === cell ? draft.zoom : (slot?.zoom ?? 1),
        };
      const next = Math.min(
        ZOOM_MAX,
        Math.max(ZOOM_MIN, cur.zoom * Math.exp(-e.deltaY * 0.0022)),
      );
      molette.current = { focal: cur.focal, zoom: next };
      setDraft({ slot: cell, focal: cur.focal, zoom: next });
      window.clearTimeout(moletteTimer.current);
      moletteTimer.current = window.setTimeout(() => {
        const w = molette.current;
        molette.current = null;
        if (w) {
          setDraft(null);
          onCrop(cell, w.focal, w.zoom);
        }
      }, 350);
    };
    el.addEventListener("wheel", surMolette, { passive: false });
    return () => el.removeEventListener("wheel", surMolette);
  });

  const roomMm = geom.w / 2 - geom.margin - geom.gutter / 2;

  /**
   * The one block of text a spread can carry — half-title, text page or
   * colophon — in the shape the screen gives it.
   *
   * Making the three one role is what pays here: the anchor and the lines
   * both come from the scene, and all that is left is a choice of DOM. The
   * half-title sets its lines one by one, because they are not all the same
   * size; the other two are one flowing block, which is also what lets a
   * text page be edited in place.
   *
   * That flowing block still spaces its lines with the screen's leading
   * rather than the print's — a difference of about a millimetre a line,
   * older than this port, and one the canvas will not inherit.
   */
  const blocDeTexte = (role: Extract<Role, { role: "text" }>) => {
    if (spread.template === GARDE_TEMPLATE) {
      return role.lines.map((l, i) => {
        // The size on screen is the print size, and the baseline is a box
        // top one size up.
        const px = Math.max(l.sizeMm * mm * 1.35, 11);
        return (
          <span
            key={i}
            className="garde-line"
            aria-hidden="true"
            style={{
              left: `${role.at.x * mm}px`,
              top: `${(role.at.y + l.dyMm) * mm - px}px`,
              fontSize: `${px}px`,
            }}
          >
            {l.text}
          </span>
        );
      });
    }
    // The colophon is read-only: the engine writes it from what it measured,
    // and typing over it would turn a statement of fact into a caption. The
    // Envoi screen is the one place it can be taken away.
    const colophon = spread.template === COLOPHON_TEMPLATE;
    const sizeMm = colophon ? COLOPHON_SIZE_MM : TEXT_SIZE_MM;
    return (
      <TextBlock
        text={spread.text ?? ""}
        at={role.at}
        mm={mm}
        sizeMm={sizeMm}
        roomMm={roomMm}
        fontPx={Math.max(sizeMm * mm * 1.35, colophon ? 11 : 13)}
        leadPx={(colophon ? COLOPHON_LEADING_MM : TEXT_LEADING_MM) * mm * 1.35}
        editing={editingText}
        onEditing={setEditingText}
        onText={spread.template === "texte" ? onText : undefined}
        objetDeScene
      />
    );
  };

  return (
    <div
      ref={paper}
      className={"paper" + (hasSelection ? " has-selection" : "")}
      style={
        {
          aspectRatio: `${trimW} / ${album.trim_mm.h}`,
          "--spread-aspect": trimW / album.trim_mm.h,
        } as React.CSSProperties
      }
      onClick={() => onSelect?.(null)}
    >
      <div
        className="media-box"
        style={{
          left: `${-album.bleed_mm * mm}px`,
          top: `${-album.bleed_mm * mm}px`,
          width: `${geom.w * mm}px`,
          height: `${geom.h * mm}px`,
        }}
      >
        {/* Painted in one element rather than thirty, when the switch says
            so. The gestures below it read the same scene through the hit
            test; nothing here decides what a spread holds. */}
        {modeCanvas && (
          <SceneCanvas
            scene={{ objects: scene.objects.filter((o) => !enEdition(o)) }}
            geom={geom}
            mm={mm}
            selected={selected}
            drop={drop}
            canvasRef={canvas}
            onPointerDown={onSelect && surPointerDown}
            onPointerMove={onSelect && surPointerMove}
            onPointerUp={onSelect && surPointerUp}
            onDoubleClick={surDoubleClic}
            onDragOver={
              (onSwap || onPlace) &&
              ((e) => {
                e.preventDefault();
                e.dataTransfer.dropEffect = "move";
                const p = enMm(e);
                setDrop(caseSous(p.x, p.y));
              })
            }
            onDragLeave={() => setDrop(null)}
            onDrop={
              onPlace &&
              ((e) => {
                e.preventDefault();
                const p = enMm(e);
                const cell = caseSous(p.x, p.y);
                setDrop(null);
                const pool = e.dataTransfer.getData("application/x-colophon-photo");
                if (cell === null || !pool) return;
                try {
                  const photo = JSON.parse(pool) as Slot;
                  if (photo.src) {
                    onPlace(cell, {
                      src: photo.src,
                      focal: photo.focal ?? [0.5, 0.42],
                    });
                  }
                } catch {
                  /* not ours */
                }
              })
            }
          />
        )}

        {/* The scene, in its own order: back to front, exactly as the PDF's
            content stream lays it down. Under the canvas renderer only what
            is being typed into stays in the DOM: a field is a field. */}
        {scene.objects.map((o, depth) => {
          const role = o.role;
          if (modeCanvas && !enEdition(o)) return null;
          switch (role.role) {
            case "photo": {
              const cell = role.cell;
              return (
                <CropPhoto
                  key={`${role.src}-${cell}`}
                  src={role.src}
                  rect={o.rect}
                  mm={mm}
                  focal={role.focal}
                  zoom={role.zoom}
                  zoomPose={spread.slots[cell]?.zoom ?? 1}
                  selected={selected === cell}
                  onSelect={
                    onSelect && (() => onSelect(selected === cell ? null : cell))
                  }
                  onSwap={onSwap && ((from) => onSwap(from, cell))}
                  onPlace={onPlace && ((photo) => onPlace(cell, photo))}
                  onDraft={(focal, zoom) => setDraft({ slot: cell, focal, zoom })}
                  onCommit={
                    onCrop &&
                    ((focal, zoom) => {
                      setDraft(null);
                      onCrop(cell, focal, zoom);
                    })
                  }
                  index={cell}
                  onSansMarge={onSansMarge}
                />
              );
            }

            // At print size, on its own baseline. The ink the scene measured
            // is what says whether the line runs past its case.
            case "photo_caption": {
              const cell = cellRects.get(role.cell);
              const over = cell !== undefined && o.rect.w > cell.w;
              return (
                <span
                  key={`cap-${role.cell}`}
                  className={"photo-caption" + (over ? " overflow" : "")}
                  style={{
                    left: `${role.at.x * mm}px`,
                    top: `${role.at.y * mm}px`,
                    maxWidth: "none",
                    fontSize: `${Math.max(PHOTO_CAPTION_SIZE_MM * mm * 1.35, 9)}px`,
                  }}
                  title={over ? t("planche.legende.deborde") : undefined}
                  // Nommée par son proxy, comme tout objet de la scène.
                  aria-hidden="true"
                >
                  {role.text}
                </span>
              );
            }

            case "chapter_caption":
              return (
                <Fragment key={`chapitre-${depth}`}>
                  {chapitre(role.text, role.at)}
                </Fragment>
              );

            case "text":
              return (
                <Fragment key={`texte-${depth}`}>{blocDeTexte(role)}</Fragment>
              );
          }
        })}

        {/* An untitled chapter has no object on the scene, and the invitation
            to title one is not a thing that prints: it is the one anchor this
            view still asks the geometry for. */}
        {!aChapitre &&
          onSpreadCaption &&
          chapitre(null, captionAnchor(spread.template, spread.slots.length, geom))}

        {/* The badges a case wears, when a canvas draws the case. The rule
            they read is the DOM renderer's own (`photos.ts::badgesDe`); what
            differs is only that there is no `<img>` here to hang them on.
            They stay in the DOM on purpose: an infobulle carries the remedy,
            and a canvas has no infobulle. */}
        {modeCanvas &&
          onSelect &&
          [...cellRects].map(([cell, r]) => {
            const slot = spread.slots[cell];
            const img = slot ? imageDe(slot.src) : null;
            if (!slot || !img) return null;
            const zoomPose = slot.zoom ?? 1;
            const zoom = draft?.slot === cell ? draft.zoom : zoomPose;
            const b = badgesDe(slot.src, img, r, mm, zoom);
            const montreZoom = selected === cell && zoomPose > 1.001;
            if (b.ppi === null && !b.dark && !montreZoom) return null;
            return (
              <div
                key={`badges-${cell}`}
                className="slot-chips"
                style={{
                  left: `${r.x * mm}px`,
                  top: `${r.y * mm}px`,
                  width: `${r.w * mm}px`,
                  height: `${r.h * mm}px`,
                }}
              >
                {(b.ppi !== null || b.dark) && (
                  <span className="slot-warns">
                    {b.ppi !== null && (
                      <span
                        className="slot-warn"
                        title={t("planche.warn.ppi", {
                          ppi: b.ppi,
                          plancher: MIN_EFFECTIVE_PPI,
                        })}
                      >
                        {b.ppi} ppi
                      </span>
                    )}
                    {b.dark && (
                      <span className="slot-warn" title={t("planche.warn.sombre")}>
                        {t("planche.warn.sombre.badge")}
                      </span>
                    )}
                  </span>
                )}
                {montreZoom && (
                  <span className="slot-zoom">
                    ×{zoom.toFixed(2).replace(".", ",")}
                  </span>
                )}
              </div>
            );
          })}

        {/* A text page with nothing written yet: same case, one page later. */}
        {!aTexte && spread.template === "texte" && (
          <TextBlock
            text=""
            at={textAnchor(geom)}
            mm={mm}
            sizeMm={TEXT_SIZE_MM}
            roomMm={roomMm}
            fontPx={Math.max(TEXT_SIZE_MM * mm * 1.35, 13)}
            leadPx={TEXT_LEADING_MM * mm * 1.35}
            editing={editingText}
            onEditing={setEditingText}
            onText={onText}
            objetDeScene={false}
          />
        )}

        {/* One focusable box per object, in reading order, laid over the
            page and invisible to the pointer. Last, so nothing paints over
            a focus ring. */}
        <SceneProxies
          scene={scene}
          mm={mm}
          trim={{
            x: album.bleed_mm,
            y: album.bleed_mm,
            w: geom.w - 2 * album.bleed_mm,
            h: geom.h - 2 * album.bleed_mm,
          }}
          selected={selected}
          planche={planche}
          edition={editingCaption || editingText}
          onActivate={activer}
          onEchap={() => onSelect?.(null)}
          onPlanche={onPlanche}
        />
      </div>
      <div className="gutter" aria-hidden="true" />
      {selectedSlot && selectedRect && paperBox && onCaption && (
        <CaptionPopover
          key={`${selectedSlot.src}-${selected}`}
          slot={selectedSlot}
          paperBox={paperBox}
          rect={selectedRect}
          bleed={album.bleed_mm}
          mm={mm}
          onCaption={(text) => onCaption(selected!, text)}
        />
      )}
    </div>
  );
}

/**
 * The caption editor of the selected photo: a popover under its case (above
 * it when the case touches the bottom of the window), an input, the EXIF
 * suggestion one click away. Clicks inside stay inside: the selection holds.
 */
function CaptionPopover({
  slot,
  paperBox,
  rect,
  bleed,
  mm,
  onCaption,
}: {
  slot: Slot;
  paperBox: DOMRect;
  rect: Rect;
  bleed: number;
  mm: number;
  onCaption: (text: string) => void;
}) {
  const [value, setValue] = useState(slot.caption ?? "");
  const [suggestion, setSuggestion] = useState<string | null>(null);

  useEffect(() => {
    setValue(slot.caption ?? "");
    setSuggestion(null);
    let alive = true;
    captionSuggestion(slot.src).then(
      (s) => alive && setSuggestion(s),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [slot]);

  const HEIGHT = 46;
  const left = Math.max(
    8,
    Math.min(
      paperBox.left + (rect.x - bleed) * mm,
      window.innerWidth - 328,
    ),
  );
  const below = paperBox.top + (rect.y + rect.h - bleed) * mm + 8;
  const top =
    below + HEIGHT + 8 > window.innerHeight
      ? paperBox.top + (rect.y - bleed) * mm - HEIGHT - 8
      : below;

  return (
    <div
      className="caption-popover"
      style={{ left, top }}
      onClick={(e) => e.stopPropagation()}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <label className="caption-popover-label">
        {t("planche.legende")}
        <input
          className="caption-popover-input"
          value={value}
          placeholder={t("planche.legende.aucune")}
          onChange={(e) => setValue(e.target.value)}
          onBlur={() => value.trim() !== (slot.caption ?? "") && onCaption(value)}
          onKeyDown={(e) => {
            // Même garde que le titre de chapitre : l'action par défaut
            // d'Entrée cliquerait la boîte à qui la fermeture rend le focus.
            if (e.key === "Enter") {
              e.preventDefault();
              e.currentTarget.blur();
            }
            if (e.key === "Escape") {
              setValue(slot.caption ?? "");
              e.currentTarget.blur();
            }
          }}
        />
      </label>
      {suggestion && !value && (
        <button
          className="link"
          onClick={() => {
            setValue(suggestion);
            onCaption(suggestion);
          }}
          title={t("planche.legende.exif")}
        >
          {t("planche.legende.proposer", { texte: suggestion })}
        </button>
      )}
    </div>
  );
}

/**
 * A flowing block of set text: the free text of a `texte` spread, or the
 * colophon. A click turns the first into a textarea in place; overlong lines
 * are underlined, never wrapped or cut for print.
 *
 * It takes the raw text rather than the scene's lines, because a blank line
 * has to hold its row here: on the scene a blank line is a `dy` the next
 * line carries, and in a CSS grid it is a row of its own.
 */
function TextBlock({
  text,
  at,
  mm,
  sizeMm,
  fontPx,
  leadPx,
  roomMm,
  editing,
  onEditing,
  onText,
  objetDeScene,
}: {
  text: string;
  /** The block's first baseline, millimetres, top-left origin. */
  at: Point;
  mm: number;
  /** Print size of a line, for the overflow measurement. */
  sizeMm: number;
  fontPx: number;
  leadPx: number;
  roomMm: number;
  /** Held above: the mouse opens the block, and so does the keyboard. */
  editing: boolean;
  onEditing: (on: boolean) => void;
  onText?: (text: string) => void;
  /** Vrai quand un objet de la scène porte ce texte : le proxy le nomme, et
   *  la copie peinte sort de l'arbre d'accessibilité. Faux pour la page de
   *  texte encore vide, qui ne double aucun proxy et doit donc être sa
   *  propre porte d'entrée. */
  objetDeScene: boolean;
}) {

  const x = at.x * mm;
  const y = at.y * mm;
  const width = roomMm * mm;

  if (editing && onText) {
    return (
      <textarea
        className="text-page-input"
        style={{
          left: `${x}px`,
          top: `${y - fontPx}px`,
          width: `${width}px`,
          fontSize: `${fontPx}px`,
          lineHeight: `${Math.max(leadPx, fontPx * 1.3)}px`,
        }}
        defaultValue={text}
        placeholder={t("planche.texte.placeholder")}
        autoFocus
        onClick={(e) => e.stopPropagation()}
        onBlur={(e) => {
          onEditing(false);
          onText(e.currentTarget.value);
        }}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") e.currentTarget.blur();
        }}
      />
    );
  }

  const lines = text.split("\n");
  return (
    <div
      className={"text-page" + (onText ? " editable" : "")}
      style={{
        left: `${x}px`,
        top: `${y - fontPx}px`,
        width: `${width}px`,
        fontSize: `${fontPx}px`,
        lineHeight: `${Math.max(leadPx, fontPx * 1.3)}px`,
      }}
      title={onText ? t("planche.texte.editer") : undefined}
      aria-hidden={objetDeScene ? true : undefined}
      role={!objetDeScene && onText ? "button" : undefined}
      tabIndex={!objetDeScene && onText ? 0 : undefined}
      aria-label={!objetDeScene && onText ? t("planche.texte.ecrire") : undefined}
      onClick={
        onText &&
        ((e) => {
          e.stopPropagation();
          onEditing(true);
        })
      }
      onKeyDown={
        !objetDeScene && onText
          ? (e) => {
              if (e.key !== "Enter" && e.key !== " ") return;
              e.preventDefault();
              e.stopPropagation();
              onEditing(true);
            }
          : undefined
      }
    >
      {text === "" ? (
        <span className="text-page-ghost">{t("planche.texte.ghost")}</span>
      ) : (
        lines.map((l, i) => (
          <span
            key={i}
            className={
              "text-page-line" +
              (measureMm(l, sizeMm) > roomMm ? " overflow" : "")
            }
          >
            {l || " "}
          </span>
        ))
      )}
    </div>
  );
}

/**
 * One photo in its case. Unselected: click selects, HTML5 drag swaps with
 * another case, a drawer photo can drop in. Selected: the pointer owns the
 * crop (drag moves the framing, ⌥ refines, wheel zooms, double-click
 * recentres on the detected focal point).
 */
function CropPhoto({
  src,
  rect,
  mm,
  focal,
  zoom,
  zoomPose,
  index,
  selected,
  onSelect,
  onSwap,
  onPlace,
  onDraft,
  onCommit,
  onSansMarge,
}: {
  src: string;
  rect: Rect;
  mm: number;
  focal: [number, number];
  /** Shown: the gesture's draft while one is running, the album's otherwise. */
  zoom: number;
  /** Stored: what album.json holds, which is what the badges speak about. */
  zoomPose: number;
  index: number;
  selected?: boolean;
  onSelect?: () => void;
  onSwap?: (from: number) => void;
  onPlace?: (photo: Slot) => void;
  onDraft: (focal: [number, number], zoom: number) => void;
  onCommit?: (focal: [number, number], zoom: number) => void;
  /** The photo fills its cell exactly and the drag has nothing to move. */
  onSansMarge?: () => void;
}) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(src));
  const [over, setOver] = useState(false);
  const img = useRef<HTMLImageElement>(null);
  const gesture = useRef<{
    id: number;
    x: number;
    y: number;
    focal: [number, number];
    moved: boolean;
    /** The « nothing to slide » notice has been given for this gesture. */
    signale: boolean;
  } | null>(null);
  // The wheel commits when it stops: one undo step per zoom burst.
  const wheelState = useRef<{ focal: [number, number]; zoom: number } | null>(null);
  const wheelTimer = useRef<number | undefined>(undefined);
  // The click that closes a crop drag must not toggle the selection.
  const justDragged = useRef(false);

  useEffect(() => {
    let alive = true;
    const hit = cachedThumb(src);
    if (hit) {
      setUrl(hit);
      return;
    }
    setUrl(undefined);
    loadThumb(src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [src]);

  // Warning badges, computed from the thumbnail already on screen (front
  // only, no engine round-trip). Resolution is only asserted when it is
  // known: a thumbnail under THUMB_SIZE was never downscaled, so its pixel
  // count is the original's. A downscaled one proves the original is
  // bigger, hence a computed ppi ABOVE the floor clears the photo but one
  // below it proves nothing, and no badge shows. The preflight, which
  // reopens the originals, remains the authority at export time.
  const [warn, setWarn] = useState<{ ppi: number | null; dark: boolean }>({
    ppi: null,
    dark: false,
  });
  // Whether this framing has any slack, kept in state because it depends on
  // the loaded image's own pixels. Feeds the tooltip; the gesture recomputes
  // it from the same function rather than reading this, so a stale render can
  // never make a drag lie.
  const [sansMarge, setSansMarge] = useState(false);
  useEffect(() => {
    const el = img.current;
    if (!el || !url) return;
    const inspect = () => {
      if (!el.naturalWidth) return;
      // The same rule the canvas renderer reads, written once.
      const b = badgesDe(src, el, rect, mm, zoom);
      setWarn({ ppi: b.ppi, dark: b.dark });
      setSansMarge(b.sansMarge);
    };
    if (el.complete) {
      inspect();
      return;
    }
    el.addEventListener("load", inspect, { once: true });
    return () => el.removeEventListener("load", inspect);
  }, [url, src, rect.w, rect.h, zoom, mm]);

  // Wheel zoom needs a non-passive listener to swallow the page scroll.
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current;
    if (!el || !selected || !onCommit) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const cur = wheelState.current ?? { focal, zoom };
      const next = Math.min(
        ZOOM_MAX,
        Math.max(ZOOM_MIN, cur.zoom * Math.exp(-e.deltaY * 0.0022)),
      );
      wheelState.current = { focal: cur.focal, zoom: next };
      onDraft(cur.focal, next);
      window.clearTimeout(wheelTimer.current);
      wheelTimer.current = window.setTimeout(() => {
        const w = wheelState.current;
        wheelState.current = null;
        if (w) onCommit(w.focal, w.zoom);
      }, 350);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  /** Pointer drag on a selected slot: move the crop window. */
  const startCrop = (e: React.PointerEvent) => {
    if (!selected || !onCommit || e.button !== 0) return;
    e.stopPropagation();
    gesture.current = {
      id: e.pointerId,
      x: e.clientX,
      y: e.clientY,
      focal: [...focal] as [number, number],
      moved: false,
      signale: false,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  const moveCrop = (e: React.PointerEvent) => {
    const g = gesture.current;
    const el = img.current;
    if (!g || g.id !== e.pointerId || !el?.naturalWidth) return;
    const { x: spanX, y: spanY } = slidingRoom(
      { w: rect.w * mm, h: rect.h * mm },
      el.naturalWidth,
      el.naturalHeight,
      zoom,
    );
    const fine = e.altKey ? 0.2 : 1;
    const dx = (e.clientX - g.x) * fine;
    const dy = (e.clientY - g.y) * fine;
    if (!g.moved && Math.abs(dx) + Math.abs(dy) < 3) return;
    g.moved = true;
    // A drag that cannot move anything is the moment to say why, and to name
    // the way out. Once per gesture: a message repeated at every pointer event
    // is noise, and noise is what teaches people to stop reading messages.
    if (spanX <= ROOM_EPSILON && spanY <= ROOM_EPSILON) {
      if (!g.signale) {
        g.signale = true;
        onSansMarge?.();
      }
      return;
    }
    const fx = spanX > 0.5 ? g.focal[0] - dx / spanX : g.focal[0];
    const fy = spanY > 0.5 ? g.focal[1] - dy / spanY : g.focal[1];
    // Une trame de recadrage, dev seulement : de l'événement de pointeur au
    // pixel. C'est la mesure qui dira si un canvas glisse mieux qu'un DOM.
    const fin = jusquAuRendu("recadrage.trame");
    onDraft(
      [Math.min(1, Math.max(0, fx)), Math.min(1, Math.max(0, fy))],
      zoom,
    );
    fin();
  };

  const endCrop = (e: React.PointerEvent) => {
    const g = gesture.current;
    if (!g || g.id !== e.pointerId) return;
    gesture.current = null;
    if (g.moved) {
      justDragged.current = true;
      onCommit?.(focal, zoom);
    }
  };

  const recentre = async (e: React.MouseEvent) => {
    if (!selected || !onCommit) return;
    e.stopPropagation();
    const f = await detectedFocal(src).catch(() => [0.5, 0.42] as [number, number]);
    onCommit([f[0], f[1]], zoom);
  };

  const editable = Boolean(onSelect);
  const style: React.CSSProperties = {
    left: `${rect.x * mm}px`,
    top: `${rect.y * mm}px`,
    width: `${rect.w * mm}px`,
    height: `${rect.h * mm}px`,
  };

  return (
    <div
      ref={box}
      className={
        "slot" +
        (editable ? " editable" : "") +
        (selected ? " selected cropping" : "") +
        (over ? " dropping" : "") +
        (zoomPose > 1.001 ? " zoomed" : "")
      }
      style={style}
      onClick={
        onSelect &&
        ((e) => {
          e.stopPropagation();
          if (justDragged.current) {
            justDragged.current = false;
            return;
          }
          if (!gesture.current) onSelect();
        })
      }
      onDoubleClick={selected ? recentre : undefined}
      onPointerDown={startCrop}
      onPointerMove={moveCrop}
      onPointerUp={endCrop}
      onPointerCancel={endCrop}
      draggable={Boolean(onSwap) && !selected}
      onDragStart={(e) => {
        e.dataTransfer.setData("text/colophon-slot", String(index));
        e.dataTransfer.effectAllowed = "move";
      }}
      onDragOver={
        (onSwap || onPlace) &&
        ((e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        })
      }
      onDragEnter={(onSwap || onPlace) && (() => setOver(true))}
      onDragLeave={(onSwap || onPlace) && (() => setOver(false))}
      onDrop={
        (onSwap || onPlace) &&
        ((e) => {
          e.preventDefault();
          setOver(false);
          const pool = e.dataTransfer.getData("application/x-colophon-photo");
          if (pool && onPlace) {
            try {
              const photo = JSON.parse(pool) as Slot;
              if (photo.src) onPlace({ src: photo.src, focal: photo.focal ?? [0.5, 0.42] });
            } catch {
              /* not ours */
            }
            return;
          }
          const from = Number(e.dataTransfer.getData("text/colophon-slot"));
          if (Number.isInteger(from) && onSwap) onSwap(from);
        })
      }
      title={
        selected
          ? sansMarge
            ? t("planche.recadrer.pleine")
            : t("planche.recadrer")
          : undefined
      }
      // Cette boîte est le dessin d'un objet de la scène, et son infobulle
      // parle à la souris : le proxy nomme la photo, et le canvas ne pose
      // aucune boîte du tout. `presentation` la retire de l'arbre sans
      // emporter les badges, qui eux existent sous les deux rendus.
      role="presentation"
    >
      {url && (
        <img
          ref={img}
          src={url}
          alt=""
          draggable={false}
          // Cover-crop plus manual zoom: object-position anchors the focal,
          // the scale around that same origin reproduces pdf.rs::crop_window
          // exactly (same fixed point, same visible window).
          style={{
            objectPosition: `${focal[0] * 100}% ${focal[1] * 100}%`,
            transform: zoom > 1.001 ? `scale(${zoom})` : undefined,
            transformOrigin: `${focal[0] * 100}% ${focal[1] * 100}%`,
          }}
        />
      )}
      {selected && zoomPose > 1.001 && (
        <span className="slot-zoom">×{zoom.toFixed(2).replace(".", ",")}</span>
      )}
      {editable && (warn.ppi !== null || warn.dark) && (
        <span className="slot-warns">
          {warn.ppi !== null && (
            <span
              className="slot-warn"
              title={t("planche.warn.ppi", {
                ppi: warn.ppi,
                plancher: MIN_EFFECTIVE_PPI,
              })}
            >
              {warn.ppi} ppi
            </span>
          )}
          {warn.dark && (
            <span
              className="slot-warn"
              title={t("planche.warn.sombre")}
            >
              {t("planche.warn.sombre.badge")}
            </span>
          )}
        </span>
      )}
    </div>
  );
}

/** Shared with the light table: the crop of one slot as CSS, the same
 *  cover + focal + scale-around-focal maths as the print (see CropPhoto). */
export function thumbCropStyle(slot: Slot): React.CSSProperties {
  const zoom = slot.zoom ?? 1;
  return {
    objectPosition: `${slot.focal[0] * 100}% ${slot.focal[1] * 100}%`,
    transform: zoom > 1.001 ? `scale(${zoom})` : undefined,
    transformOrigin: `${slot.focal[0] * 100}% ${slot.focal[1] * 100}%`,
  };
}
