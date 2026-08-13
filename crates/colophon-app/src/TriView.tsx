// The sorting view: everything the album does not show, grouped by the
// reason it was set aside, each photo one click away from a rescue. Fed by
// curation.json; photos removed by hand in the book view surface here too,
// derived from the thumbnail index.

import { useEffect, useMemo, useRef, useState } from "react";
import { TriEntry } from "./edits";
import { cachedThumb, loadThumb } from "./thumbs";

/** Sections in display order, with human labels. */
const REASONS: [string, string][] = [
  ["retiree", "Retirées à la main"],
  ["hors_budget", "Hors budget : bonnes photos, album plein"],
  ["meme_moment", "Même moment, quasi la même photo"],
  ["doublon", "Doublons de rafale ou de scène"],
  ["jumeau", "Quasi identiques"],
  ["parasite", "Parasites : captures, images reçues"],
];

export function TriView({
  entries,
  selected,
  onSelect,
  onRescue,
}: {
  entries: TriEntry[];
  selected: string | null;
  onSelect: (src: string | null) => void;
  onRescue: (entry: TriEntry) => void;
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
        <p>Rien à trier : toutes les photos du dossier sont dans l'album.</p>
      </div>
    );
  }

  return (
    <div className="tri" onClick={() => onSelect(null)}>
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
      {selected && (
        <button
          className="tri-rescue"
          onClick={(e) => {
            e.stopPropagation();
            onRescue();
          }}
        >
          Repêcher
        </button>
      )}
    </figure>
  );
}

/**
 * A thumbnail that only fetches once scrolled into view: the sorting view
 * lists hundreds of photos, the blob-URL pool is bounded, and most cells
 * are never seen.
 */
function LazyThumb({ src }: { src: string }) {
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
