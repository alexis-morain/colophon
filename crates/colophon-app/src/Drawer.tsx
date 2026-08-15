// The photo drawer of the book view: every photo the album does not show,
// one drag away from any case. Two tabs — unplaced (good photos the budget
// or a hand removal set aside) and discarded (curation calls) — with the
// reason on hover. The pool mechanics of SmartAlbums, kept to one strip:
// the sorting view remains the place for a full audit.

import { useState } from "react";
import { TriEntry } from "./edits";
import { LazyThumb } from "./TriView";

/** Reasons that read as « good photo, just not placed ». */
const UNPLACED = new Set(["hors_budget", "retiree"]);

const REASON_LABELS: Record<string, string> = {
  retiree: "retirée à la main",
  rejetee: "rejetée dans votre logiciel photo",
  hors_budget: "hors budget : bonne photo, album plein",
  meme_moment: "même moment, quasi la même photo",
  doublon: "doublon de rafale ou de scène",
  jumeau: "quasi identique à une photo gardée",
  panorama: "panorama : trop large pour une page",
  definition: "définition trop faible pour ce format",
  parasite: "parasite : capture, image reçue",
};

export function Drawer({
  entries,
  open,
  onToggle,
}: {
  entries: TriEntry[];
  open: boolean;
  onToggle: () => void;
}) {
  const [tab, setTab] = useState<"non-placees" | "ecartees">("non-placees");
  const unplaced = entries.filter((e) => UNPLACED.has(e.reason));
  const discarded = entries.filter((e) => !UNPLACED.has(e.reason));
  const list = tab === "non-placees" ? unplaced : discarded;

  return (
    <section className={"drawer" + (open ? " open" : "")}>
      <header className="drawer-bar">
        <button
          className="drawer-toggle"
          onClick={onToggle}
          title="P"
          aria-expanded={open}
        >
          <span className="drawer-chevron" aria-hidden="true">
            {open ? "▾" : "▴"}
          </span>
          Photos en réserve · {entries.length}
        </button>
        {open && (
          <span className="drawer-tabs" role="tablist">
            <button
              role="tab"
              aria-selected={tab === "non-placees"}
              className={"drawer-tab" + (tab === "non-placees" ? " active" : "")}
              onClick={() => setTab("non-placees")}
            >
              Non placées · {unplaced.length}
            </button>
            <button
              role="tab"
              aria-selected={tab === "ecartees"}
              className={"drawer-tab" + (tab === "ecartees" ? " active" : "")}
              onClick={() => setTab("ecartees")}
            >
              Écartées · {discarded.length}
            </button>
          </span>
        )}
        {open && (
          <span className="drawer-hint">
            glissez une photo sur une case du livre pour l'y placer
          </span>
        )}
      </header>
      {open && (
        <div className="drawer-strip" role="list">
          {list.length === 0 ? (
            <p className="drawer-empty">
              {tab === "non-placees"
                ? "Aucune photo en attente : tout ce qui mérite l'album y est."
                : "Rien d'écarté par la curation."}
            </p>
          ) : (
            list.map((e) => (
              <figure
                key={e.src}
                role="listitem"
                className="drawer-cell"
                draggable
                onDragStart={(ev) => {
                  ev.dataTransfer.setData(
                    "application/x-colophon-photo",
                    JSON.stringify({ src: e.src, focal: e.focal }),
                  );
                  ev.dataTransfer.effectAllowed = "copy";
                }}
                title={`${e.src.split("/").pop()} · ${
                  REASON_LABELS[e.reason] ?? e.reason
                }${e.kept ? ` (gardée : ${e.kept.split("/").pop()})` : ""}`}
              >
                <LazyThumb src={e.src} />
              </figure>
            ))
          )}
        </div>
      )}
    </section>
  );
}
