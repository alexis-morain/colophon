// The photo drawer of the book view: every photo the album does not show,
// one drag away from any case. Two tabs — unplaced (good photos the budget
// or a hand removal set aside) and discarded (curation calls) — with the
// reason on hover. The pool mechanics of SmartAlbums, kept to one strip:
// the sorting view remains the place for a full audit.

import { useState } from "react";
import { TriEntry } from "./edits";
import { Chevron } from "./icons";
import { reasonPhrase } from "./reasons";
import { LazyThumb } from "./TriView";

/** Reasons that read as « good photo, just not placed ». */
const UNPLACED = new Set(["hors_budget", "retiree"]);

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
            <Chevron dir={open ? "down" : "up"} />
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
            glissez une photo sur une case du livre pour l’y placer
          </span>
        )}
      </header>
      {open && (
        <div className="drawer-strip" role="list">
          {list.length === 0 ? (
            <p className="drawer-empty">
              {tab === "non-placees"
                ? "Aucune photo en attente : tout ce qui mérite l’album y est."
                : "Rien d’écarté par la curation."}
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
                title={`${e.src.split("/").pop()} · ${reasonPhrase(e.reason)}${
                  e.kept ? ` (gardée : ${e.kept.split("/").pop()})` : ""
                }`}
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
