// One spread, rendered at the trimmed size the reader will hold. The media
// canvas (bleed included) sits behind, offset so the bleed falls outside the
// visible page, exactly like a trimmed print. When the edit callbacks are
// given, photos become selectable and draggable; the view stays a pure
// reader without them.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Album,
  CAPTION_SIZE_MM,
  captionAnchor,
  mediaCanvas,
  slotsFor,
  Spread,
} from "./album";
import { cachedThumb, loadThumb } from "./thumbs";

export function SpreadView({
  album,
  spread,
  selected,
  onSelect,
  onSwap,
}: {
  album: Album;
  spread: Spread;
  selected?: number | null;
  onSelect?: (slot: number | null) => void;
  onSwap?: (a: number, b: number) => void;
}) {
  const paper = useRef<HTMLDivElement>(null);
  const [mm, setMm] = useState(1);

  const trimW = album.trim_mm.w * 2;
  const canvas = mediaCanvas(album);
  const rects = slotsFor(spread.template, spread.slots.length, canvas);
  const caption = captionAnchor(spread.template, spread.slots.length, canvas);

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

  return (
    <div
      ref={paper}
      className="paper"
      style={
        {
          aspectRatio: `${trimW} / ${album.trim_mm.h}`,
          "--spread-aspect": trimW / album.trim_mm.h,
        } as React.CSSProperties
      }
      onClick={() => onSelect?.(null)}
    >
      <div
        className="canvas"
        style={{
          left: `${-album.bleed_mm * mm}px`,
          top: `${-album.bleed_mm * mm}px`,
          width: `${canvas.w * mm}px`,
          height: `${canvas.h * mm}px`,
        }}
      >
        {spread.slots.map((slot, i) => {
          const r = rects[i];
          if (!r) return null;
          return (
            <Photo
              key={`${slot.src}-${i}`}
              src={slot.src}
              focal={slot.focal}
              selected={selected === i}
              onSelect={onSelect && (() => onSelect(selected === i ? null : i))}
              onSwap={onSwap && ((from) => onSwap(from, i))}
              index={i}
              style={{
                left: `${r.x * mm}px`,
                top: `${r.y * mm}px`,
                width: `${r.w * mm}px`,
                height: `${r.h * mm}px`,
              }}
            />
          );
        })}

        {spread.caption && (
          <span
            className="caption"
            style={{
              left: `${caption.x * mm}px`,
              top: `${caption.y * mm}px`,
              fontSize: `${CAPTION_SIZE_MM * mm * 1.35}px`,
            }}
          >
            {spread.caption}
          </span>
        )}
      </div>
      <div className="gutter" aria-hidden="true" />
    </div>
  );
}

function Photo({
  src,
  focal,
  style,
  index,
  selected,
  onSelect,
  onSwap,
}: {
  src: string;
  focal: [number, number];
  style: React.CSSProperties;
  index: number;
  selected?: boolean;
  onSelect?: () => void;
  onSwap?: (from: number) => void;
}) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(src));
  const [over, setOver] = useState(false);

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

  const editable = Boolean(onSelect);
  return (
    <div
      className={
        "slot" +
        (editable ? " editable" : "") +
        (selected ? " selected" : "") +
        (over ? " dropping" : "")
      }
      style={style}
      onClick={
        onSelect &&
        ((e) => {
          e.stopPropagation();
          onSelect();
        })
      }
      draggable={Boolean(onSwap)}
      onDragStart={(e) => {
        e.dataTransfer.setData("text/colophon-slot", String(index));
        e.dataTransfer.effectAllowed = "move";
      }}
      onDragOver={
        onSwap &&
        ((e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
        })
      }
      onDragEnter={onSwap && (() => setOver(true))}
      onDragLeave={onSwap && (() => setOver(false))}
      onDrop={
        onSwap &&
        ((e) => {
          e.preventDefault();
          setOver(false);
          const from = Number(e.dataTransfer.getData("text/colophon-slot"));
          if (Number.isInteger(from)) onSwap(from);
        })
      }
    >
      {url && (
        <img
          src={url}
          alt=""
          // Same anchor convention as the renderer: focal y runs from the top.
          style={{ objectPosition: `${focal[0] * 100}% ${focal[1] * 100}%` }}
        />
      )}
    </div>
  );
}
