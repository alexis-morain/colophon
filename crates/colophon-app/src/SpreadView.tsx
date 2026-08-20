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
  DARK_MEAN_LUMA,
  MIN_EFFECTIVE_PPI,
  THUMB_SIZE,
  ZOOM_MAX,
  ZOOM_MIN,
  captionAnchor,
  effectivePpi,
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
import { fontLoaded, measureMm } from "./font";
import { t } from "./i18n";
import { jusquAuRendu } from "./mesure";
import { Point, Role, sceneOf } from "./scene";
import { cachedThumb, loadThumb, meanLuma } from "./thumbs";

/** A crop being adjusted: values shown before they land on the undo stack. */
type CropDraft = { slot: number; focal: [number, number]; zoom: number };

/** Under half a pixel each way, no gesture can move anything. */
const ROOM_EPSILON = 0.5;

export function SpreadView({
  album,
  spread,
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
}: {
  album: Album;
  spread: Spread;
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
}) {
  const paper = useRef<HTMLDivElement>(null);
  const [mm, setMm] = useState(1);
  const [draft, setDraft] = useState<CropDraft | null>(null);
  const [editingCaption, setEditingCaption] = useState(false);

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
  const scene = sceneOf(spread, geom, measureMm);
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
  const chapitre = (text: string | null, at: Point) =>
    editingCaption && onSpreadCaption ? (
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
          if (e.key === "Enter") e.currentTarget.blur();
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

  const aChapitre = scene.objects.some((o) => o.role.role === "chapter_caption");
  const aTexte = scene.objects.some((o) => o.role.role === "text");

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
        onText={spread.template === "texte" ? onText : undefined}
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
        {/* The scene, in its own order: back to front, exactly as the PDF's
            content stream lays it down. */}
        {scene.objects.map((o, depth) => {
          const role = o.role;
          switch (role.role) {
            case "photo": {
              const cell = role.cell;
              const d = draft?.slot === cell ? draft : null;
              return (
                <CropPhoto
                  key={`${role.src}-${cell}`}
                  src={role.src}
                  rect={o.rect}
                  mm={mm}
                  focal={d?.focal ?? role.focal}
                  zoom={d?.zoom ?? role.zoom}
                  zoomPose={role.zoom}
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
            onText={onText}
          />
        )}
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
            if (e.key === "Enter") e.currentTarget.blur();
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
  onText,
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
  onText?: (text: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  useEffect(() => setEditing(false), [text === ""]);

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
          setEditing(false);
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
      onClick={
        onText &&
        ((e) => {
          e.stopPropagation();
          setEditing(true);
        })
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
      const known = Math.max(el.naturalWidth, el.naturalHeight) < THUMB_SIZE;
      const p = effectivePpi(rect, el.naturalWidth, el.naturalHeight, zoom);
      const luma = meanLuma(src, el);
      setWarn({
        ppi: known && p < MIN_EFFECTIVE_PPI ? Math.round(p) : null,
        dark: luma !== undefined && luma < DARK_MEAN_LUMA,
      });
      const room = slidingRoom(
        { w: rect.w * mm, h: rect.h * mm },
        el.naturalWidth,
        el.naturalHeight,
        zoom,
      );
      setSansMarge(room.x <= ROOM_EPSILON && room.y <= ROOM_EPSILON);
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
