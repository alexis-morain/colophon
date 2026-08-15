// The sorting view: everything the album does not show, grouped by the
// reason it was set aside, each photo one click away from a rescue. Fed by
// curation.json; photos removed by hand in the book view surface here too,
// derived from the thumbnail index.

import { useEffect, useMemo, useRef, useState } from "react";
import { TriEntry } from "./edits";
import { REASONS, reasonLabel } from "./reasons";
import { cachedThumb, loadThumb } from "./thumbs";

export function TriView({
  entries,
  selected,
  onSelect,
  onRescue,
  onRevue,
}: {
  entries: TriEntry[];
  selected: string | null;
  onSelect: (src: string | null) => void;
  onRescue: (entry: TriEntry) => void;
  onRevue: () => void;
}) {
  const sections = useMemo(() => {
    const by = new Map<string, TriEntry[]>();
    for (const e of entries) {
      const list = by.get(e.reason) ?? [];
      list.push(e);
      by.set(e.reason, list);
    }
    return REASONS.filter(([key]) => by.get(key)?.length).map(
      ([key, label]) => ({ key, label, list: by.get(key)! }),
    );
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="tri tri-empty">
        <p>Rien à trier : toutes les photos du dossier sont dans l’album.</p>
      </div>
    );
  }

  return (
    <div className="tri" onClick={() => onSelect(null)}>
      <div className="tri-head">
        <p className="tri-lede">
          {entries.length} photo{entries.length > 1 ? "s" : ""} hors de
          l’album, chacune avec sa raison. Un double-clic repêche.
        </p>
        <button
          className="cta small"
          onClick={(e) => {
            e.stopPropagation();
            onRevue();
          }}
        >
          Passer en revue&ensp;<kbd>Entrée</kbd>
        </button>
      </div>
      {sections.map(({ key, label, list }) => (
        <section key={key} className="tri-section">
          <h2>
            {label}
            <span className="tri-count">{list.length}</span>
          </h2>
          <div className="tri-grid">
            {list.map((e) => (
              <Cell
                key={e.src}
                entry={e}
                selected={selected === e.src}
                onSelect={() => onSelect(selected === e.src ? null : e.src)}
                onRescue={() => onRescue(e)}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function Cell({
  entry,
  selected,
  onSelect,
  onRescue,
}: {
  entry: TriEntry;
  selected: boolean;
  onSelect: () => void;
  onRescue: () => void;
}) {
  const name = entry.src.split("/").pop() ?? entry.src;
  return (
    <figure
      className={"tri-cell" + (selected ? " selected" : "")}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onRescue();
      }}
      title={entry.kept ? `${name}, gardée : ${entry.kept.split("/").pop()}` : name}
    >
      <LazyThumb src={entry.src} />
    </figure>
  );
}

/**
 * The keyboard review, taken from the culling tools photographers use: one
 * discarded photo at a time, full screen, its reason printed on the image.
 * Arrows browse, R rescues, X confirms the discard and moves on, Escape
 * leaves. The keys live in App's central handler; this component draws.
 */
export function RevueView({
  entries,
  index,
  status,
  onIndex,
  onRescue,
  onClose,
}: {
  entries: TriEntry[];
  index: number;
  status: string | null;
  onIndex: (i: number) => void;
  onRescue: (entry: TriEntry) => void;
  onClose: () => void;
}) {
  const i = Math.max(0, Math.min(index, entries.length - 1));
  const entry = entries[i];
  const [url, setUrl] = useState<string | undefined>(() =>
    cachedThumb(entry.src),
  );

  useEffect(() => {
    let alive = true;
    setUrl(cachedThumb(entry.src));
    loadThumb(entry.src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    // The next photo loads behind the current one, so → never waits.
    const next = entries[i + 1];
    if (next) loadThumb(next.src).catch(() => {});
    return () => {
      alive = false;
    };
  }, [entry.src, entries, i]);

  const name = entry.src.split("/").pop() ?? entry.src;
  return (
    <div className="revue">
      <figure className="revue-stage">
        {url && <img src={url} alt={name} />}
        <span className="revue-reason">{reasonLabel(entry.reason)}</span>
        <span className="revue-pos">
          {i + 1} / {entries.length}
        </span>
      </figure>
      <footer className="revue-foot">
        <span className="revue-name">
          {name}
          {entry.kept
            ? `, gardée à sa place : ${entry.kept.split("/").pop()}`
            : ""}
        </span>
        {status && <span className="revue-status">{status}</span>}
        <span className="revue-keys">
          <button className="link" onClick={() => onRescue(entry)}>
            Repêcher&ensp;<kbd>R</kbd>
          </button>
          <button className="link" onClick={() => onIndex(i + 1)}>
            Écart confirmé&ensp;<kbd>X</kbd>
          </button>
          <span className="revue-hint">
            <kbd>←</kbd> <kbd>→</kbd> parcourir
          </span>
          <button className="link" onClick={onClose}>
            Sortir&ensp;<kbd>Échap</kbd>
          </button>
        </span>
      </footer>
    </div>
  );
}

/**
 * A thumbnail that only fetches once scrolled into view: the sorting view
 * lists hundreds of photos, the blob-URL pool is bounded, and most cells
 * are never seen. The drawer and the light table lean on it too.
 */
export function LazyThumb({ src }: { src: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(src));

  useEffect(() => {
    if (url) return;
    const el = ref.current;
    if (!el) return;
    let alive = true;
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        io.disconnect();
        loadThumb(src).then(
          (u) => alive && setUrl(u),
          () => {},
        );
      },
      { rootMargin: "200px" },
    );
    io.observe(el);
    return () => {
      alive = false;
      io.disconnect();
    };
  }, [src, url]);

  return (
    <div ref={ref} className="tri-thumb">
      {url && <img src={url} alt="" loading="lazy" />}
    </div>
  );
}
