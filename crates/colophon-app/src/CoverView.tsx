// The cover editor: back cover, spine, front cover, laid flat like the
// printed sheet. Title and subtitle edit in place, the front photo comes
// from the album and recrops like any case (drag to frame, wheel to zoom),
// the back text is optional. The spine width is computed and shown; its
// formula is provisional until Cloudprinter answers, and nothing but the
// display depends on it.

import { useEffect, useRef, useState } from "react";
import { Album, Cover, Slot, ZOOM_MAX, ZOOM_MIN, spineMm } from "./album";
import { LazyThumb } from "./TriView";
import { cachedThumb, loadThumb } from "./thumbs";

export function CoverView({
  album,
  onCover,
}: {
  album: Album;
  onCover: (cover: Cover) => void;
}) {
  const cover: Cover = album.cover ?? { title: album.title };
  // Text fields edit a local form and land on the undo stack at blur:
  // one ⌘Z per field touched, not one per keystroke.
  const [form, setForm] = useState<Cover>(cover);
  const [picking, setPicking] = useState(false);
  const spine = spineMm(album.spreads.length);

  useEffect(() => {
    setForm(album.cover ?? { title: album.title });
  }, [album]);

  // Escape closes the photo picker before anything else reacts.
  useEffect(() => {
    if (!picking) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setPicking(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [picking]);

  // Every distinct photo the album shows, for the picker.
  const srcs = Array.from(
    new Set(album.spreads.flatMap((s) => s.slots.map((sl) => sl.src))),
  );

  /** Commit right away (photo picks, crops). */
  const set = (patch: Partial<Cover>) => onCover({ ...form, ...patch });
  /** Commit the text fields as they stand. */
  const commitForm = () => {
    const c = album.cover ?? { title: album.title };
    if (
      form.title !== c.title ||
      (form.subtitle ?? "") !== (c.subtitle ?? "") ||
      (form.back_text ?? "") !== (c.back_text ?? "")
    ) {
      onCover(form);
    }
  };

  const sheetAspect = (album.trim_mm.w * 2 + spine) / album.trim_mm.h;
  return (
    <div
      className="cover"
      style={{ "--cover-aspect": sheetAspect } as React.CSSProperties}
    >
      <div
        className="cover-sheet"
        style={{ aspectRatio: `${album.trim_mm.w * 2 + spine} / ${album.trim_mm.h}` }}
      >
        {/* Back cover: the quatrième, optional. */}
        <div className="cover-back" style={{ flex: album.trim_mm.w }}>
          <textarea
            className="cover-back-text"
            value={form.back_text ?? ""}
            placeholder="Quatrième de couverture (optionnelle) : un mot, une dédicace, un été."
            onChange={(e) => setForm({ ...form, back_text: e.target.value })}
            onBlur={commitForm}
          />
        </div>

        {/* Spine: computed width, title along it. */}
        <div
          className="cover-spine"
          style={{ flex: spine }}
          title={`Dos calculé : ${spine.toFixed(1).replace(".", ",")} mm (provisoire, en attente de l'imprimeur)`}
        >
          <span className="cover-spine-title">{form.title || album.title}</span>
        </div>

        {/* Front cover: photo + title block. */}
        <div className="cover-front" style={{ flex: album.trim_mm.w }}>
          {cover.photo ? (
            <CoverPhoto
              photo={cover.photo}
              onChange={(photo) => set({ photo })}
            />
          ) : (
            <button className="cover-photo-empty" onClick={() => setPicking(true)}>
              Choisir la photo de couverture…
            </button>
          )}
          <div className="cover-titles" onClick={(e) => e.stopPropagation()}>
            <input
              className="cover-title-input"
              value={form.title}
              placeholder={album.title}
              onChange={(e) => setForm({ ...form, title: e.target.value })}
              onBlur={commitForm}
              onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              aria-label="Titre de la couverture"
            />
            <input
              className="cover-subtitle-input"
              value={form.subtitle ?? ""}
              placeholder="sous-titre (optionnel)"
              onChange={(e) => setForm({ ...form, subtitle: e.target.value })}
              onBlur={commitForm}
              onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              aria-label="Sous-titre"
            />
          </div>
          {cover.photo && (
            <button
              className="cover-photo-change"
              onClick={() => setPicking(true)}
              title="Choisir une autre photo de l'album"
            >
              changer la photo
            </button>
          )}
        </div>
      </div>

      <p className="cover-note">
        Dos calculé : {spine.toFixed(1).replace(".", ",")} mm pour{" "}
        {album.spreads.length * 2} pages · valeur provisoire, la formule de
        l'imprimeur la remplacera.
      </p>

      {picking && (
        <div className="cover-picker" role="listbox">
          <header className="cover-picker-bar">
            <span>Photo de couverture, parmi l'album</span>
            <button className="link" onClick={() => setPicking(false)}>
              fermer (Échap)
            </button>
          </header>
          <div className="cover-picker-grid">
            {srcs.map((src) => (
              <button
                key={src}
                role="option"
                aria-selected={cover.photo?.src === src}
                className={
                  "cover-picker-cell" +
                  (cover.photo?.src === src ? " active" : "")
                }
                onClick={() => {
                  set({ photo: { src, focal: [0.5, 0.42] } });
                  setPicking(false);
                }}
              >
                <LazyThumb src={src} />
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** The front-cover photo: full-bleed, recroppable with the same gestures
 *  as a case (drag to frame, ⌥ refines, wheel to zoom). */
function CoverPhoto({
  photo,
  onChange,
}: {
  photo: Slot;
  onChange: (photo: Slot) => void;
}) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(photo.src));
  const [draft, setDraft] = useState<Slot | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const img = useRef<HTMLImageElement>(null);
  const gesture = useRef<{ id: number; x: number; y: number; focal: [number, number] } | null>(null);
  const wheelDraft = useRef<Slot | null>(null);
  const wheelTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let alive = true;
    if (!cachedThumb(photo.src)) setUrl(undefined);
    loadThumb(photo.src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [photo.src]);

  useEffect(() => setDraft(null), [photo]);

  const shown = draft ?? photo;
  const zoom = shown.zoom ?? 1;

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const cur = wheelDraft.current ?? shown;
      const z = Math.min(
        ZOOM_MAX,
        Math.max(ZOOM_MIN, (cur.zoom ?? 1) * Math.exp(-e.deltaY * 0.0022)),
      );
      const next = { ...cur, zoom: z };
      wheelDraft.current = next;
      setDraft(next);
      window.clearTimeout(wheelTimer.current);
      wheelTimer.current = window.setTimeout(() => {
        const w = wheelDraft.current;
        wheelDraft.current = null;
        if (w) onChange(w);
      }, 350);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  return (
    <div
      ref={box}
      className="cover-photo"
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        gesture.current = {
          id: e.pointerId,
          x: e.clientX,
          y: e.clientY,
          focal: [...shown.focal] as [number, number],
        };
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        const g = gesture.current;
        const el = img.current;
        const boxEl = box.current;
        if (!g || g.id !== e.pointerId || !el?.naturalWidth || !boxEl) return;
        const w = boxEl.clientWidth;
        const h = boxEl.clientHeight;
        const s = Math.max(w / el.naturalWidth, h / el.naturalHeight) * zoom;
        const spanX = el.naturalWidth * s - w;
        const spanY = el.naturalHeight * s - h;
        const fine = e.altKey ? 0.2 : 1;
        const fx = spanX > 0.5 ? g.focal[0] - ((e.clientX - g.x) * fine) / spanX : g.focal[0];
        const fy = spanY > 0.5 ? g.focal[1] - ((e.clientY - g.y) * fine) / spanY : g.focal[1];
        setDraft({
          ...shown,
          focal: [Math.min(1, Math.max(0, fx)), Math.min(1, Math.max(0, fy))],
        });
      }}
      onPointerUp={(e) => {
        const g = gesture.current;
        if (!g || g.id !== e.pointerId) return;
        gesture.current = null;
        if (draft) onChange(draft);
      }}
      title="Glisser pour recadrer · molette pour zoomer · ⌥ affine"
    >
      {url && (
        <img
          ref={img}
          src={url}
          alt=""
          draggable={false}
          style={{
            objectPosition: `${shown.focal[0] * 100}% ${shown.focal[1] * 100}%`,
            transform: zoom > 1.001 ? `scale(${zoom})` : undefined,
            transformOrigin: `${shown.focal[0] * 100}% ${shown.focal[1] * 100}%`,
          }}
        />
      )}
    </div>
  );
}
