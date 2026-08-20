// One spread, rendered at the trimmed size the reader will hold. The media
// box (bleed included) sits behind, offset so the bleed falls outside the
// visible page, exactly like a trimmed print. When the edit callbacks are
// given, photos become selectable and draggable; the view stays a pure
// reader without them.
//
// The selected photo is a crop editor: dragging inside the case moves the
// framing (⌥ refines), the wheel zooms past the fill, a double-click
// recentres on the detected focal. Gestures work on a local draft and land
// on the undo stack once, at the end of the gesture.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Album,
  CAPTION_SIZE_MM,
  PHOTO_CAPTION_DROP_MM,
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
  slotsFor,
  textAnchor,
  colophonAnchor,
  COLOPHON_TEMPLATE,
  COLOPHON_SIZE_MM,
  COLOPHON_LEADING_MM,
  gardeAnchor,
  gardeLayout,
  gardePlace,
  GARDE_TEMPLATE,
} from "./album";
import { captionSuggestion, detectedFocal } from "./bridge";
import { t } from "./i18n";
import { cachedThumb, loadThumb, meanLuma } from "./thumbs";

/** A crop being adjusted: values shown before they land on the undo stack. */
type CropDraft = { slot: number; focal: [number, number]; zoom: number };

/** Measure a string at a given CSS font, for overflow signalling. */
const measure = (() => {
  let ctx: CanvasRenderingContext2D | null = null;
  return (text: string, font: string): number => {
    if (!ctx) ctx = document.createElement("canvas").getContext("2d");
    if (!ctx) return 0;
    ctx.font = font;
    return ctx.measureText(text).width;
  };
})();

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
}) {
  const paper = useRef<HTMLDivElement>(null);
  const [mm, setMm] = useState(1);
  const [draft, setDraft] = useState<CropDraft | null>(null);
  const [editingCaption, setEditingCaption] = useState(false);

  const trimW = album.trim_mm.w * 2;
  const geom = spreadGeometry(album);
  const rects = slotsFor(spread.template, spread.slots.length, geom);
  const caption = captionAnchor(spread.template, spread.slots.length, geom);

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
  // milliseconds), measure everything again.
  const [fontReady, setFontReady] = useState(false);
  useEffect(() => {
    let alive = true;
    document.fonts.load('100px "Source Sans 3"').then(
      () => alive && setFontReady(true),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, []);

  // Photo captions wider than their slot, and text lines wider than the
  // page: named to the reader, never cut.
  useEffect(() => {
    if (!onOverflow) return;
    const problems: string[] = [];
    spread.slots.forEach((slot, i) => {
      const r = rects[i];
      if (!slot.caption || !r) return;
      // Below the trimmed page (full-bleed slots): the caption would print
      // in the bleed and be cut off entirely.
      if (r.y + r.h + PHOTO_CAPTION_DROP_MM > geom.h - 4) {
        problems.push(t("deborde.legende.horspage", { i: i + 1 }));
        return;
      }
      const wMm = measureMm(slot.caption, PHOTO_CAPTION_SIZE_MM);
      if (wMm > r.w) {
        problems.push(
          t("deborde.legende.longue", { i: i + 1, mm: Math.ceil(wMm - r.w) }),
        );
      }
    });
    if (spread.template === "texte" && spread.text) {
      const room = geom.w / 2 - geom.margin - geom.gutter / 2;
      const over = spread.text
        .split("\n")
        .filter((l) => measureMm(l, TEXT_SIZE_MM) > room).length;
      if (over > 0) {
        problems.push(
          over > 1 ? t("deborde.lignes", { n: over }) : t("deborde.ligne.une"),
        );
      }
    }
    // The half-title fits its title by shrinking it, and builds its town
    // line to the page: nothing here overflows unless album.json was
    // repaired by hand, which is precisely when saying so is worth it.
    if (spread.template === GARDE_TEMPLATE && spread.text) {
      const room = gardePlace(geom);
      const over = gardeLayout(spread.text, room, measureMm).some(
        (l) => measureMm(l.texte, l.tailleMm) > room + 0.01,
      );
      if (over) {
        problems.push(t("deborde.garde"));
      }
    }
    onOverflow(problems[0] ?? null);
  }, [spread, rects, geom, mm, onOverflow, fontReady]);

  const textAt = textAnchor(geom);
  const colophonAt = colophonAnchor(geom);
  const gardeAt = gardeAnchor(geom);

  // The caption popover anchors under the selected case, in viewport
  // coordinates (position: fixed): it may hang below the sheet without
  // being clipped by the paper's overflow.
  const hasSelection = selected !== null && selected !== undefined;
  const selectedSlot = hasSelection ? (spread.slots[selected] ?? null) : null;
  const selectedRect = hasSelection ? (rects[selected] ?? null) : null;
  const paperBox = paper.current?.getBoundingClientRect() ?? null;

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
        {spread.slots.map((slot, i) => {
          const r = rects[i];
          if (!r) return null;
          const d = draft?.slot === i ? draft : null;
          return (
            <CropPhoto
              key={`${slot.src}-${i}`}
              slot={slot}
              rect={r}
              mm={mm}
              focal={d?.focal ?? slot.focal}
              zoom={d?.zoom ?? slot.zoom ?? 1}
              selected={selected === i}
              onSelect={onSelect && (() => onSelect(selected === i ? null : i))}
              onSwap={onSwap && ((from) => onSwap(from, i))}
              onPlace={onPlace && ((photo) => onPlace(i, photo))}
              onDraft={(focal, zoom) => setDraft({ slot: i, focal, zoom })}
              onCommit={
                onCrop &&
                ((focal, zoom) => {
                  setDraft(null);
                  onCrop(i, focal, zoom);
                })
              }
              index={i}
            />
          );
        })}

        {/* Photo captions, at print size and position. */}
        {spread.slots.map((slot, i) => {
          const r = rects[i];
          if (!slot.caption || !r) return null;
          const over = measureMm(slot.caption, PHOTO_CAPTION_SIZE_MM) > r.w;
          return (
            <span
              key={`cap-${i}`}
              className={"photo-caption" + (over ? " overflow" : "")}
              style={{
                left: `${r.x * mm}px`,
                top: `${(r.y + r.h + PHOTO_CAPTION_DROP_MM) * mm}px`,
                maxWidth: "none",
                fontSize: `${Math.max(PHOTO_CAPTION_SIZE_MM * mm * 1.35, 9)}px`,
              }}
              title={over ? t("planche.legende.deborde") : undefined}
            >
              {slot.caption}
            </span>
          );
        })}

        {/* Chapter caption: readable in place, renamable in place. */}
        {editingCaption && onSpreadCaption ? (
          <input
            className="caption caption-input"
            style={{
              left: `${caption.x * mm}px`,
              top: `${caption.y * mm}px`,
              fontSize: `${Math.max(CAPTION_SIZE_MM * mm * 1.35, 13)}px`,
            }}
            defaultValue={spread.caption ?? ""}
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
                e.currentTarget.value = spread.caption ?? "";
                e.currentTarget.blur();
              }
            }}
          />
        ) : (
          (spread.caption || onSpreadCaption) && (
            <span
              className={
                "caption" +
                (onSpreadCaption ? " editable" : "") +
                (spread.caption ? "" : " ghost")
              }
              style={{
                left: `${caption.x * mm}px`,
                top: `${caption.y * mm}px`,
                fontSize: `${CAPTION_SIZE_MM * mm * 1.35}px`,
              }}
              title={
                !spread.caption && proposition
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
              {spread.caption ?? proposition ?? t("planche.chapitre.ghost")}
            </span>
          )
        )}

        {/* Free-text page: the text in place, editable in place. */}
        {spread.template === "texte" && (
          <TextBlock
            text={spread.text ?? ""}
            x={textAt.x * mm}
            y={textAt.y * mm}
            width={(geom.w / 2 - geom.margin - geom.gutter / 2) * mm}
            fontPx={Math.max(TEXT_SIZE_MM * mm * 1.35, 13)}
            leadPx={TEXT_LEADING_MM * mm * 1.35}
            roomMm={geom.w / 2 - geom.margin - geom.gutter / 2}
            onText={onText}
          />
        )}

        {/* The half-title, read-only like the colophon: the dates and the
            towns are what the machine measured, and the title is edited in
            the bar, where renaming the book also rewrites this line. */}
        {spread.template === GARDE_TEMPLATE &&
          gardeLayout(spread.text ?? "", gardePlace(geom), measureMm).map(
            (l, i) => {
              // Same reading as the text pages: the size on screen is the
              // print size, and the baseline is a box top one size up.
              const px = Math.max(l.tailleMm * mm * 1.35, 11);
              return (
                <span
                  key={i}
                  className="garde-line"
                  style={{
                    left: `${gardeAt.x * mm}px`,
                    top: `${(gardeAt.y + l.dyMm) * mm - px}px`,
                    fontSize: `${px}px`,
                  }}
                >
                  {l.texte}
                </span>
              );
            },
          )}

        {/* The colophon: the same block, quieter and lower, and read-only.
            The engine writes it from what it measured; typing over it would
            turn a statement of fact into a caption. The Envoi screen is the
            one place it can be taken away. */}
        {spread.template === COLOPHON_TEMPLATE && (
          <TextBlock
            text={spread.text ?? ""}
            x={colophonAt.x * mm}
            y={colophonAt.y * mm}
            width={(geom.w / 2 - geom.margin - geom.gutter / 2) * mm}
            fontPx={Math.max(COLOPHON_SIZE_MM * mm * 1.35, 11)}
            leadPx={COLOPHON_LEADING_MM * mm * 1.35}
            roomMm={geom.w / 2 - geom.margin - geom.gutter / 2}
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

/** Width of a string in spread millimetres at a print size in mm: measured
 *  at a big fixed size (glyph widths scale linearly), then scaled down.
 *  The face is the one the PDF embeds: the overflow warning and the print
 *  agree on every glyph. */
function measureMm(text: string, sizeMm: number): number {
  return (measure(text, '100px "Source Sans 3", sans-serif') * sizeMm) / 100;
}

/**
 * The free text of a `texte` spread. A click turns it into a textarea in
 * place; overlong lines are underlined, never wrapped or cut for print.
 */
function TextBlock({
  text,
  x,
  y,
  width,
  fontPx,
  leadPx,
  roomMm,
  onText,
}: {
  text: string;
  x: number;
  y: number;
  width: number;
  fontPx: number;
  leadPx: number;
  roomMm: number;
  onText?: (text: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  useEffect(() => setEditing(false), [text === ""]);

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
        <span className="text-page-ghost">Page de texte : cliquer pour écrire.</span>
      ) : (
        lines.map((l, i) => (
          <span
            key={i}
            className={
              "text-page-line" +
              (measureMm(l, TEXT_SIZE_MM) > roomMm ? " overflow" : "")
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
  slot,
  rect,
  mm,
  focal,
  zoom,
  index,
  selected,
  onSelect,
  onSwap,
  onPlace,
  onDraft,
  onCommit,
}: {
  slot: Slot;
  rect: Rect;
  mm: number;
  focal: [number, number];
  zoom: number;
  index: number;
  selected?: boolean;
  onSelect?: () => void;
  onSwap?: (from: number) => void;
  onPlace?: (photo: Slot) => void;
  onDraft: (focal: [number, number], zoom: number) => void;
  onCommit?: (focal: [number, number], zoom: number) => void;
}) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(slot.src));
  const [over, setOver] = useState(false);
  const img = useRef<HTMLImageElement>(null);
  const gesture = useRef<{
    id: number;
    x: number;
    y: number;
    focal: [number, number];
    moved: boolean;
  } | null>(null);
  // The wheel commits when it stops: one undo step per zoom burst.
  const wheelState = useRef<{ focal: [number, number]; zoom: number } | null>(null);
  const wheelTimer = useRef<number | undefined>(undefined);
  // The click that closes a crop drag must not toggle the selection.
  const justDragged = useRef(false);

  useEffect(() => {
    let alive = true;
    const hit = cachedThumb(slot.src);
    if (hit) {
      setUrl(hit);
      return;
    }
    setUrl(undefined);
    loadThumb(slot.src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [slot.src]);

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
  useEffect(() => {
    const el = img.current;
    if (!el || !url) return;
    const inspect = () => {
      if (!el.naturalWidth) return;
      const known = Math.max(el.naturalWidth, el.naturalHeight) < THUMB_SIZE;
      const p = effectivePpi(rect, el.naturalWidth, el.naturalHeight, zoom);
      const luma = meanLuma(slot.src, el);
      setWarn({
        ppi: known && p < MIN_EFFECTIVE_PPI ? Math.round(p) : null,
        dark: luma !== undefined && luma < DARK_MEAN_LUMA,
      });
    };
    if (el.complete) {
      inspect();
      return;
    }
    el.addEventListener("load", inspect, { once: true });
    return () => el.removeEventListener("load", inspect);
  }, [url, slot.src, rect.w, rect.h, zoom]);

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
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  const moveCrop = (e: React.PointerEvent) => {
    const g = gesture.current;
    const el = img.current;
    if (!g || g.id !== e.pointerId || !el?.naturalWidth) return;
    const iw = el.naturalWidth;
    const ih = el.naturalHeight;
    const w = rect.w * mm;
    const h = rect.h * mm;
    const s = Math.max(w / iw, h / ih) * zoom;
    const spanX = iw * s - w; // how far the image can slide, in px
    const spanY = ih * s - h;
    const fine = e.altKey ? 0.2 : 1;
    const dx = (e.clientX - g.x) * fine;
    const dy = (e.clientY - g.y) * fine;
    if (!g.moved && Math.abs(dx) + Math.abs(dy) < 3) return;
    g.moved = true;
    const fx = spanX > 0.5 ? g.focal[0] - dx / spanX : g.focal[0];
    const fy = spanY > 0.5 ? g.focal[1] - dy / spanY : g.focal[1];
    onDraft(
      [Math.min(1, Math.max(0, fx)), Math.min(1, Math.max(0, fy))],
      zoom,
    );
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
    const f = await detectedFocal(slot.src).catch(() => [0.5, 0.42] as [number, number]);
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
        ((slot.zoom ?? 1) > 1.001 ? " zoomed" : "")
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
          ? t("planche.recadrer")
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
      {selected && (slot.zoom ?? 1) > 1.001 && (
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
