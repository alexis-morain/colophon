// The light table: every spread as a miniature, on one scrollable grid.
// Reordering is a drag, duplicating a keystroke, a photo-less or text
// spread one click away: the sequencing work that makes or breaks the book
// happens here, nearly for free. Images only load when their cell scrolls
// into view; the blob pool stays bounded.

import { useEffect, useRef, useState } from "react";
import { Album, Spread, mediaCanvas, slotsFor } from "./album";
import { thumbCropStyle } from "./SpreadView";
import { cachedThumb, loadThumb } from "./thumbs";

export function PlanchesView({
  album,
  current,
  onSelect,
  onOpen,
  onMove,
  onLock,
}: {
  album: Album;
  /** Index of the highlighted cell; -1 is the cover, page zero. */
  current: number;
  onSelect: (at: number) => void;
  /** Double-click: open this spread (or the cover, -1) in the book view. */
  onOpen: (at: number) => void;
  onMove: (from: number, to: number) => void;
  onLock: (at: number) => void;
}) {
  const [dropAt, setDropAt] = useState<number | null>(null);

  return (
    <div className="planches" role="list">
      <CoverCell
        album={album}
        current={current === -1}
        onSelect={() => onSelect(-1)}
        onOpen={() => onOpen(-1)}
      />
      {album.spreads.map((spread, i) => (
        <PlancheCell
          key={i}
          album={album}
          spread={spread}
          index={i}
          current={i === current}
          dropping={dropAt === i}
          onSelect={() => onSelect(i)}
          onOpen={() => onOpen(i)}
          onLock={() => onLock(i)}
          onDragStartCell={(e) => {
            e.dataTransfer.setData("text/colophon-spread", String(i));
            e.dataTransfer.effectAllowed = "move";
          }}
          onDragOverCell={(e) => {
            if (!e.dataTransfer.types.includes("text/colophon-spread")) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
            setDropAt(i);
          }}
          onDragLeaveCell={() => setDropAt((d) => (d === i ? null : d))}
          onDropCell={(e) => {
            e.preventDefault();
            setDropAt(null);
            const from = Number(e.dataTransfer.getData("text/colophon-spread"));
            if (Number.isInteger(from) && from !== i) onMove(from, i);
          }}
        />
      ))}
    </div>
  );
}

function PlancheCell({
  album,
  spread,
  index,
  current,
  dropping,
  onSelect,
  onOpen,
  onLock,
  onDragStartCell,
  onDragOverCell,
  onDragLeaveCell,
  onDropCell,
}: {
  album: Album;
  spread: Spread;
  index: number;
  current: boolean;
  dropping: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onLock: () => void;
  onDragStartCell: (e: React.DragEvent) => void;
  onDragOverCell: (e: React.DragEvent) => void;
  onDragLeaveCell: () => void;
  onDropCell: (e: React.DragEvent) => void;
}) {
  return (
    <figure
      role="listitem"
      className={
        "planche-cell" +
        (current ? " current" : "") +
        (dropping ? " dropping" : "")
      }
      draggable
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
      onDragStart={onDragStartCell}
      onDragOver={onDragOverCell}
      onDragLeave={onDragLeaveCell}
      onDrop={onDropCell}
      title={
        (spread.caption ? `${spread.caption} · ` : "") +
        `planche ${index + 1} · glisser pour déplacer, double-clic pour ouvrir`
      }
    >
      <MiniSpread album={album} spread={spread} />
      <figcaption className="planche-meta">
        <span className="planche-num">{index + 1}</span>
        {spread.caption && <span className="planche-chapter">{spread.caption}</span>}
        <span className="planche-flags">
          {spread.edited && (
            <span
              className="badge-edited"
              title="Éditée à la main : survit à toute recomposition"
            />
          )}
          <button
            className={"lock" + (spread.locked ? " locked" : "")}
            onClick={(e) => {
              e.stopPropagation();
              onLock();
            }}
            title={
              spread.locked
                ? "Figée : survit à toute recomposition. Cliquer pour libérer (⌘L)"
                : "Figer cette planche face aux recompositions (⌘L)"
            }
            aria-pressed={spread.locked ?? false}
          >
            <LockGlyph open={!spread.locked} />
          </button>
        </span>
      </figcaption>
    </figure>
  );
}

/**
 * The cover as the light table's page zero: the whole book starts here, so
 * the whole book shows here. Front panel only, at the page's own aspect;
 * it neither drags nor receives drops, a cover has one possible place.
 */
function CoverCell({
  album,
  current,
  onSelect,
  onOpen,
}: {
  album: Album;
  current: boolean;
  onSelect: () => void;
  onOpen: () => void;
}) {
  const cover = album.cover ?? { title: album.title };
  return (
    <figure
      role="listitem"
      className={"planche-cell planche-cover" + (current ? " current" : "")}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
      title="Couverture · double-clic pour l’ouvrir"
    >
      <div
        className="mini-cover"
        style={{ aspectRatio: `${album.trim_mm.w} / ${album.trim_mm.h}` }}
      >
        {cover.photo && <MiniImg slot={cover.photo} />}
        <span className="mini-cover-title">{cover.title || album.title}</span>
      </div>
      <figcaption className="planche-meta">
        <span className="planche-num">C</span>
        <span className="planche-chapter">Couverture</span>
      </figcaption>
    </figure>
  );
}

/** A minimal padlock, drawn rather than emoji'd. */
export function LockGlyph({ open }: { open: boolean }) {
  return (
    <svg viewBox="0 0 10 12" width="10" height="12" aria-hidden="true">
      <rect x="1" y="5.5" width="8" height="6" fill="currentColor" />
      <path
        d={open ? "M 2.8 5.5 V 3.2 A 2.2 2.2 0 0 1 7.2 3.2 V 4.2" : "M 2.8 5.5 V 3.2 A 2.2 2.2 0 0 1 7.2 3.2 V 5.5"}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        transform={open ? "translate(1.6 -1.4) rotate(24 5 3.5)" : undefined}
      />
    </svg>
  );
}

/**
 * One spread at postage size: real geometry, real crops, images gated by
 * an IntersectionObserver so sixty planches cost only what is on screen.
 */
function MiniSpread({ album, spread }: { album: Album; spread: Spread }) {
  const ref = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const canvas = mediaCanvas(album);
  const rects = slotsFor(spread.template, spread.slots.length, canvas);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisible(true);
          io.disconnect();
        }
      },
      { rootMargin: "300px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className="mini-spread"
      style={{ aspectRatio: `${canvas.w} / ${canvas.h}` }}
    >
      {spread.slots.map((slot, i) => {
        const r = rects[i];
        if (!r) return null;
        return (
          <span
            key={i}
            className="mini-slot"
            style={{
              left: `${(r.x / canvas.w) * 100}%`,
              top: `${(r.y / canvas.h) * 100}%`,
              width: `${(r.w / canvas.w) * 100}%`,
              height: `${(r.h / canvas.h) * 100}%`,
            }}
          >
            {visible && <MiniImg slot={slot} />}
          </span>
        );
      })}
      {/* Les planches de texte se lisent dans la grille : sans ça, la garde
          et le colophon y sont deux rectangles vides. */}
      {spread.text !== undefined && (
        <span className="mini-text" aria-hidden="true">
          {spread.text.trim() ? spread.text : "texte"}
        </span>
      )}
      <span className="mini-fold" aria-hidden="true" />
    </div>
  );
}

function MiniImg({ slot }: { slot: { src: string; focal: [number, number]; zoom?: number } }) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(slot.src));
  useEffect(() => {
    let alive = true;
    if (!cachedThumb(slot.src)) setUrl(undefined);
    loadThumb(slot.src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [slot.src]);
  return url ? <img src={url} alt="" style={thumbCropStyle(slot)} /> : null;
}
