import { useCallback, useEffect, useRef, useState } from "react";
import {
  buildAlbum,
  cancelBuild,
  cancelExport,
  captionSuggestion,
  confirmDialog,
  exportPdf,
  fetchCuration,
  FormatPreset,
  inTauri,
  listFormats,
  onBuildProgress,
  openAlbum as openAlbumAt,
  pickAlbumFolder,
  pickPhotosFolder,
  recomposeAlbum,
  saveAlbum,
} from "./bridge";
import {
  Album,
  Discard,
  mediaCanvas,
  OpenedAlbum,
  Slot,
  slotsFor,
  spineMm,
  Spread,
} from "./album";
import {
  changeTemplate,
  duplicateSpread,
  insertSpread,
  moveBlocker,
  movePhoto,
  moveSpread,
  placePhoto,
  removePhoto,
  removeSpread,
  rescuePhoto,
  setCover,
  setSlotCaption,
  setSlotCrop,
  setSpreadCaption,
  setSpreadText,
  spreadOf,
  swapPhotos,
  toggleLock,
  triEntries,
  TriEntry,
} from "./edits";
import { SpreadView } from "./SpreadView";
import { TemplatePicker } from "./TemplatePicker";
import { TriView } from "./TriView";
import { Drawer } from "./Drawer";
import { PlanchesView, LockGlyph } from "./PlanchesView";
import { CoverView } from "./CoverView";
import { cachedThumb, loadThumb, resetThumbs } from "./thumbs";
import "./styles.css";

/** Full album snapshots: a 50-spread album is a few tens of kilobytes. */
type History = { album: Album; past: Album[]; future: Album[] };
const HISTORY_CAP = 50;

type View = "livre" | "tri" | "planches";

/** In the book view, index -1 is the cover. */
const COVER = -1;

export default function App() {
  const [opened, setOpened] = useState<OpenedAlbum | null>(null);
  const [hist, setHist] = useState<History | null>(null);
  const [savedAlbum, setSavedAlbum] = useState<Album | null>(null);
  const [index, setIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [building, setBuilding] = useState<string[] | null>(null);
  const [busyTitle, setBusyTitle] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  const [view, setView] = useState<View>("livre");
  const [curation, setCuration] = useState<Discard[]>([]);
  const [triSelected, setTriSelected] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [overflow, setOverflow] = useState<string | null>(null);

  const album = hist?.album ?? null;
  const total = album?.spreads.length ?? 0;
  const dirty = album !== null && album !== savedAlbum;

  const adopt = useCallback((result: OpenedAlbum) => {
    resetThumbs();
    setOpened(result);
    setHist({ album: result.album, past: [], future: [] });
    setSavedAlbum(result.album);
    setIndex(0);
    setSelected(null);
    setTriSelected(null);
    setView("livre");
    setError(null);
    setStatus(null);
  }, []);

  const openAlbum = useCallback(async () => {
    const picked = await pickAlbumFolder();
    if (picked === null) return;
    try {
      const result = await openAlbumAt(picked);
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      setError(String(e));
    }
  }, [adopt]);

  /** Push an edited album onto the history. No-op edits stay off the stack. */
  const apply = useCallback((edit: (album: Album) => Album) => {
    setHist((h) => {
      if (!h) return h;
      const next = edit(h.album);
      if (next === h.album) return h;
      return {
        album: next,
        past: [...h.past.slice(-(HISTORY_CAP - 1)), h.album],
        future: [],
      };
    });
  }, []);

  const undo = useCallback(() => {
    setSelected(null);
    setHist((h) => {
      if (!h || h.past.length === 0) return h;
      return {
        album: h.past[h.past.length - 1],
        past: h.past.slice(0, -1),
        future: [h.album, ...h.future],
      };
    });
  }, []);

  const redo = useCallback(() => {
    setSelected(null);
    setHist((h) => {
      if (!h || h.future.length === 0) return h;
      return {
        album: h.future[0],
        past: [...h.past.slice(-(HISTORY_CAP - 1)), h.album],
        future: h.future.slice(1),
      };
    });
  }, []);

  // Saving reads the freshest history through a ref: ⌘S from inside a
  // caption field blurs the field first, and the commit that blur applies
  // must be part of what lands on disk.
  const histRef = useRef<History | null>(null);
  histRef.current = hist;
  const save = useCallback(async () => {
    const h = histRef.current;
    if (!h) return false;
    try {
      await saveAlbum(h.album);
      setSavedAlbum(h.album);
      setStatus("Enregistré");
      return true;
    } catch (e) {
      setStatus(String(e));
      return false;
    }
  }, []);

  const regenPdf = useCallback(async () => {
    if (rendering || !hist || !(await save())) return;
    setRendering(true);
    setStatus("Rendu du PDF d’impression…");
    try {
      const dest = await exportPdf(hist.album.title, (done, total) =>
        setStatus(`Rendu à 300 dpi : ${done}/${total} photos…`),
      );
      const dos = spineMm(hist.album.spreads.length)
        .toFixed(1)
        .replace(".", ",");
      setStatus(
        dest
          ? `PDF enregistré : ${dest} · dos ${dos} mm (provisoire)`
          : "Enregistrement annulé",
      );
    } catch (e) {
      setStatus(
        String(e).includes("export annulé")
          ? "Export annulé, aucun fichier écrit"
          : String(e),
      );
    } finally {
      setRendering(false);
    }
  }, [save, rendering, hist]);

  /** Build an album from a photo folder, streaming the engine's progress. */
  const createAlbum = useCallback(async (
    dir: string,
    format: string,
    spreads: number,
    title: string | null,
  ) => {
    setBuilding([]);
    setBusyTitle(null);
    setError(null);
    // Every line is kept: the counter lines drive the progress bar, the
    // named stages feed the visible log.
    const off = await onBuildProgress((line) =>
      setBuilding((b) => (b ? [...b, line] : [line])),
    );
    try {
      const result = await buildAlbum(dir, format, spreads, title);
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      if (String(e).includes("annulée")) setStatus("Composition annulée");
      else setError(String(e));
    } finally {
      off();
      setBuilding(null);
    }
  }, [adopt]);

  /**
   * Recompose the album in place. Edited and locked spreads survive
   * verbatim, so the button is safe; it still resets ⌘Z, which is the one
   * thing worth a confirmation.
   */
  const recompose = useCallback(async () => {
    if (!hist || building) return;
    if (
      !(await confirmDialog(
        "Recomposer l'album ? Les planches éditées à la main ou verrouillées " +
          "sont conservées telles quelles, les autres sont recomposées. " +
          "L'historique d'annulation repart de zéro.",
      ))
    ) {
      return;
    }
    if (dirty && !(await save())) return;
    setBusyTitle(hist.album.title);
    setBuilding([]);
    const off = await onBuildProgress((line) =>
      setBuilding((b) => (b ? [...b, line] : [line])),
    );
    try {
      const result = await recomposeAlbum();
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
      setStatus("Album recomposé, planches éditées conservées");
    } catch (e) {
      if (String(e).includes("annulée")) setStatus("Recomposition annulée");
      else setError(String(e));
    } finally {
      off();
      setBuilding(null);
      setBusyTitle(null);
    }
  }, [hist, building, dirty, save, adopt]);

  /** Back to the creation screen. Unsaved work asks before dying. */
  const closeAlbum = useCallback(async () => {
    if (
      dirty &&
      !(await confirmDialog(
        "Des modifications ne sont pas enregistrées. Fermer quand même ?",
      ))
    ) {
      return;
    }
    setHist(null);
    setOpened(null);
    setSavedAlbum(null);
    setCuration([]);
    setSelected(null);
    setTriSelected(null);
    setView("livre");
    setStatus(null);
    setError(null);
  }, [dirty]);

  /** Bring a discarded photo back, next to the photo that beat it. */
  const rescue = useCallback(
    (entry: TriEntry) => {
      if (!album) return;
      const anchorByWinner = entry.kept ? spreadOf(album, entry.kept) : -1;
      const anchor = anchorByWinner >= 0 ? anchorByWinner : Math.max(index, 0);
      const result = rescuePhoto(
        album,
        { src: entry.src, focal: entry.focal },
        anchor,
      );
      if (!result) {
        setStatus(
          `Aucune place autour de la planche ${anchor + 1} : libérez une case ou changez un gabarit`,
        );
        return;
      }
      setTriSelected(null);
      apply(() => result.album);
      setIndex(result.at);
      setStatus(`Repêchée sur la planche ${result.at + 1}`);
    },
    [album, index, apply],
  );

  /** A drawer photo lands in a case; the displaced one joins the drawer. */
  const place = useCallback(
    (slot: number, photo: Slot) => {
      if (!album || index < 0) return;
      const before = album.spreads[index]?.slots[slot]?.src;
      const next = placePhoto(album, index, slot, photo);
      if (next === album) {
        setStatus("Déjà sur cette planche : deux fois la même photo serait un doublon");
        return;
      }
      apply(() => next);
      setStatus(
        before
          ? "Photo placée · l'ancienne repart dans la réserve"
          : "Photo placée",
      );
    },
    [album, index, apply],
  );

  // A removed spread can leave the position past the end.
  useEffect(() => {
    if (total > 0 && index >= total) setIndex(total - 1);
  }, [total, index]);

  // The selection belongs to one spread only.
  useEffect(() => setSelected(null), [index]);

  // Unsaved work guards the window, in the app and in the dev browser alike.
  useEffect(() => {
    if (!dirty) return;
    const guard = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);

  // Transient status line; errors stay put.
  useEffect(() => {
    if (!status || status.length > 60) return;
    const t = setTimeout(() => setStatus(null), 4000);
    return () => clearTimeout(t);
  }, [status]);

  // In a plain browser the album comes from the dev server: open it straight
  // away on arrival. Once only: « Nouveau » must reach the welcome screen in
  // the harness too, that is where the creation flow gets styled.
  const autoOpened = useRef(false);
  useEffect(() => {
    if (inTauri || opened || autoOpened.current) return;
    autoOpened.current = true;
    void openAlbum();
  }, [opened, openAlbum]);

  // Neighbouring spreads are fetched ahead so a page turn never flashes empty.
  useEffect(() => {
    if (!album) return;
    for (const i of [index + 1, index - 1, index + 2]) {
      album.spreads[i]?.slots.forEach((s) => loadThumb(s.src).catch(() => {}));
    }
  }, [album, index]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A focused control owns the keyboard: an input takes the letters, a
      // button takes space and enter (standard activation). App-level
      // chords still pass: ⌘S from inside a caption field must save.
      const t = e.target as HTMLElement | null;
      const key = e.key.toLowerCase();
      if (t && /^(INPUT|SELECT|TEXTAREA|BUTTON)$/.test(t.tagName)) {
        const appChord = e.metaKey && ["s", "o", "1", "2", "3"].includes(key);
        if (!appChord) return;
        if (key === "s") {
          // Blur commits the field being edited; save once that landed.
          e.preventDefault();
          t.blur();
          setTimeout(() => void save(), 0);
          return;
        }
        t.blur();
      }
      if (e.metaKey && key === "o") {
        e.preventDefault();
        void openAlbum();
        return;
      }
      if (e.metaKey && key === "s") {
        e.preventDefault();
        void save();
        return;
      }
      if (e.metaKey && key === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
        return;
      }
      if (e.metaKey && (key === "1" || key === "2" || key === "3")) {
        e.preventDefault();
        setView(key === "1" ? "livre" : key === "2" ? "tri" : "planches");
        setSelected(null);
        setTriSelected(null);
        return;
      }
      // The sorting view keeps the global shortcuts, nothing spread-bound.
      if (view === "tri") {
        if (e.key === "Escape") setTriSelected(null);
        return;
      }
      if (!total || !album) return;

      // Spread manipulation, book view and light table alike.
      if (e.metaKey && key === "d" && index >= 0) {
        e.preventDefault();
        apply((a) => duplicateSpread(a, index));
        setIndex(index + 1);
        setStatus(`Planche ${index + 1} dupliquée`);
        return;
      }
      if (e.metaKey && key === "l" && index >= 0) {
        e.preventDefault();
        const was = album.spreads[index]?.locked;
        apply((a) => toggleLock(a, index));
        setStatus(was ? "Planche libérée" : "Planche figée : elle survivra à toute recomposition");
        return;
      }

      if (view === "planches") {
        if ((e.key === "Backspace" || e.key === "Delete") && index >= 0) {
          e.preventDefault();
          apply((a) => removeSpread(a, index));
          setStatus(`Planche ${index + 1} supprimée (⌘Z la ramène)`);
          return;
        }
        if (e.key === "Escape") return;
        const step = (d: number) =>
          setIndex((i) => Math.min(total - 1, Math.max(0, i + d)));
        if (e.key === "ArrowRight") step(1);
        if (e.key === "ArrowLeft") step(-1);
        if (e.key === "Enter") setView("livre");
        return;
      }

      if (
        e.metaKey &&
        e.shiftKey &&
        (e.key === "ArrowRight" || e.key === "ArrowLeft") &&
        selected !== null &&
        index >= 0
      ) {
        e.preventDefault();
        const to = index + (e.key === "ArrowRight" ? 1 : -1);
        if (to < 0 || to >= total) return;
        const blocked = moveBlocker(album, index, selected, to);
        if (blocked === "target_full") {
          setStatus(`Planche ${to + 1} pleine : aucun gabarit n'accepte une photo de plus`);
        } else if (blocked === "source_breaks") {
          setStatus("Refusé : il faudrait sacrifier une autre photo de cette planche");
        } else if (blocked === null) {
          setSelected(null);
          apply((a) => movePhoto(a, index, selected, to));
          setStatus(`Photo envoyée sur la planche ${to + 1}`);
        }
        return;
      }

      // Crop keys on the selected photo: ⌥flèches déplacent le cadrage,
      // + / − zooment, 0 revient au remplissage exact.
      if (selected !== null && index >= 0) {
        const slot = album.spreads[index]?.slots[selected];
        if (slot) {
          const zoom = slot.zoom ?? 1;
          if (e.altKey && e.key.startsWith("Arrow")) {
            e.preventDefault();
            const d = 0.02;
            const [fx, fy] = slot.focal;
            const focal: [number, number] =
              e.key === "ArrowLeft"
                ? [fx - d, fy]
                : e.key === "ArrowRight"
                  ? [fx + d, fy]
                  : e.key === "ArrowUp"
                    ? [fx, fy - d]
                    : [fx, fy + d];
            apply((a) => setSlotCrop(a, index, selected, focal, zoom));
            return;
          }
          if (e.key === "+" || e.key === "=") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, zoom + 0.1));
            return;
          }
          if (e.key === "-" || e.key === "_") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, zoom - 0.1));
            return;
          }
          if (e.key === "0") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, 1));
            setStatus("Zoom remis au remplissage exact");
            return;
          }
        }
      }

      if ((e.key === "Backspace" || e.key === "Delete") && selected !== null && index >= 0) {
        e.preventDefault();
        setSelected(null);
        apply((a) => removePhoto(a, index, selected));
        return;
      }
      if (e.key === "Escape") {
        setSelected(null);
        return;
      }
      if (key === "p" && !e.metaKey) {
        setDrawerOpen((o) => !o);
        return;
      }
      const step = (d: number) =>
        setIndex((i) => Math.min(total - 1, Math.max(COVER, i + d)));
      switch (e.key) {
        case "ArrowRight":
        case "ArrowDown":
        case " ":
          e.preventDefault();
          step(1);
          break;
        case "ArrowLeft":
        case "ArrowUp":
          e.preventDefault();
          step(-1);
          break;
        case "Home":
          setIndex(0);
          break;
        case "End":
          setIndex(total - 1);
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [total, openAlbum, save, undo, redo, apply, index, selected, album, view]);

  if (!album || total === 0 || building) {
    return (
      <Empty
        onOpen={openAlbum}
        onCreate={createAlbum}
        building={building}
        busyTitle={busyTitle}
        error={error}
        onCancelBuild={() => void cancelBuild()}
      />
    );
  }

  const onCover = index === COVER && view === "livre";
  const spread = onCover ? null : album.spreads[Math.min(index, total - 1)];
  const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
  const triEntry = entries.find((e) => e.src === triSelected) ?? null;
  const selectedSlot =
    spread && selected !== null ? (spread.slots[selected] ?? null) : null;

  return (
    <div className="app">
      <Bar
        album={album}
        spread={spread}
        dirty={dirty}
        canUndo={(hist?.past.length ?? 0) > 0}
        canRedo={(hist?.future.length ?? 0) > 0}
        view={view}
        triCount={entries.length}
        onView={(v) => {
          setView(v);
          setSelected(null);
          setTriSelected(null);
          if (v !== "livre" && index === COVER) setIndex(0);
        }}
        onTemplate={(t) => apply((a) => changeTemplate(a, index, t))}
        onLock={
          spread
            ? () => {
                const was = spread.locked;
                apply((a) => toggleLock(a, index));
                setStatus(
                  was
                    ? "Planche libérée"
                    : "Planche figée : elle survivra à toute recomposition",
                );
              }
            : undefined
        }
        onUndo={undo}
        onRedo={redo}
        onSave={() => void save()}
        onPdf={inTauri ? () => void regenPdf() : undefined}
        pdfBusy={rendering}
        onRecompose={inTauri ? () => void recompose() : undefined}
        onOpen={openAlbum}
        onClose={closeAlbum}
      />
      {view === "tri" ? (
        <TriView
          entries={entries}
          selected={triSelected}
          onSelect={setTriSelected}
          onRescue={rescue}
        />
      ) : view === "planches" ? (
        <PlanchesView
          album={album}
          current={Math.max(index, 0)}
          onSelect={setIndex}
          onOpen={(at) => {
            setIndex(at);
            setView("livre");
          }}
          onMove={(from, to) => {
            apply((a) => moveSpread(a, from, to));
            setIndex(to);
            setStatus(`Planche déplacée en position ${to + 1}`);
          }}
          onLock={(at) => apply((a) => toggleLock(a, at))}
        />
      ) : (
        <main className="stage">
          <div className="turn" key={index}>
            {onCover ? (
              <CoverView
                album={album}
                onCover={(c) => apply((a) => setCover(a, c))}
              />
            ) : (
              spread && (
                <SpreadView
                  album={album}
                  spread={spread}
                  selected={selected}
                  onSelect={setSelected}
                  onSwap={(a, b) => apply((al) => swapPhotos(al, index, a, b))}
                  onPlace={place}
                  onCrop={(slot, focal, zoom) =>
                    apply((a) => setSlotCrop(a, index, slot, focal, zoom))
                  }
                  onSpreadCaption={(c) =>
                    apply((a) => setSpreadCaption(a, index, c))
                  }
                  onText={(text) => apply((a) => setSpreadText(a, index, text))}
                  onOverflow={setOverflow}
                />
              )
            )}
          </div>
        </main>
      )}
      {view === "livre" && !onCover && (
        <Drawer
          entries={entries}
          open={drawerOpen}
          onToggle={() => setDrawerOpen((o) => !o)}
        />
      )}
      {view === "tri" ? (
        <TriFoot
          entry={triEntry}
          status={status}
          onRescue={rescue}
          onShowSpread={(src) => {
            const at = spreadOf(album, src);
            if (at < 0) return;
            setView("livre");
            setIndex(at);
            setTriSelected(null);
          }}
        />
      ) : view === "planches" ? (
        <PlanchesFoot
          album={album}
          index={Math.max(index, 0)}
          status={status}
          onInsert={(kind) => {
            const at = Math.max(index, 0);
            apply((a) => insertSpread(a, at, kind));
            setIndex(at + 1);
            setStatus(
              kind === "vide"
                ? "Planche vide insérée : une respiration"
                : "Planche de texte insérée : double-clic pour l'ouvrir et écrire",
            );
          }}
          onDuplicate={() => {
            const at = Math.max(index, 0);
            apply((a) => duplicateSpread(a, at));
            setIndex(at + 1);
          }}
          onRemove={() => {
            const at = Math.max(index, 0);
            apply((a) => removeSpread(a, at));
            setStatus(`Planche ${at + 1} supprimée (⌘Z la ramène)`);
          }}
        />
      ) : (
        <BookFoot
          album={album}
          index={index}
          total={total}
          status={status}
          overflow={overflow}
          rendering={rendering}
          onCancelExport={() => void cancelExport()}
          selectedSlot={selectedSlot}
          onCaption={
            selected !== null && index >= 0
              ? (text) => apply((a) => setSlotCaption(a, index, selected, text))
              : undefined
          }
          onSeek={(i) => setIndex(Math.min(total - 1, Math.max(COVER, i)))}
          onMoveSpread={(from, to) => {
            apply((a) => moveSpread(a, from, to));
            setIndex(to);
            setStatus(`Planche déplacée en position ${to + 1}`);
          }}
        />
      )}
      {opened && !opened.root_present && (
        <p className="warn">
          Dossier photo introuvable ({album.root}). L'aperçu tourne sur le cache
          de vignettes, l'export pleine résolution ne marchera pas.
        </p>
      )}
    </div>
  );
}

function Bar({
  album,
  spread,
  dirty,
  canUndo,
  canRedo,
  view,
  triCount,
  onView,
  onTemplate,
  onLock,
  onUndo,
  onRedo,
  onSave,
  onPdf,
  pdfBusy,
  onRecompose,
  onOpen,
  onClose,
}: {
  album: Album;
  spread: Spread | null;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  view: View;
  triCount: number;
  onView: (v: View) => void;
  onTemplate: (t: string) => void;
  onLock?: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onPdf?: () => void;
  pdfBusy?: boolean;
  onRecompose?: () => void;
  onOpen: () => void;
  onClose: () => void;
}) {
  const photoSpread =
    spread && spread.template !== "vide" && spread.template !== "texte";
  return (
    <header className="bar">
      <h1>{album.title}</h1>
      <p className="meta">
        <span className="views" role="tablist">
          <button
            className={"view-tab" + (view === "livre" ? " active" : "")}
            onClick={() => onView("livre")}
            title="⌘1"
          >
            Livre
          </button>
          <button
            className={"view-tab" + (view === "tri" ? " active" : "")}
            onClick={() => onView("tri")}
            title="⌘2"
          >
            Tri · {triCount}
          </button>
          <button
            className={"view-tab" + (view === "planches" ? " active" : "")}
            onClick={() => onView("planches")}
            title="⌘3"
          >
            Planches
          </button>
        </span>
        {view === "livre" && spread && photoSpread && (
          <TemplatePicker album={album} spread={spread} onPick={onTemplate} />
        )}
        {view === "livre" && spread && (
          <span className="spread-flags">
            {spread.edited && (
              <span
                className="badge-edited"
                title="Éditée à la main : survit à toute recomposition"
              />
            )}
            {onLock && (
              <button
                className={"lock" + (spread.locked ? " locked" : "")}
                onClick={onLock}
                aria-pressed={spread.locked ?? false}
                title={
                  spread.locked
                    ? "Figée : survit à toute recomposition. Cliquer pour libérer (⌘L)"
                    : "Figer cette planche face aux recompositions (⌘L)"
                }
              >
                <LockGlyph open={!spread.locked} />
              </button>
            )}
          </span>
        )}
      </p>
      <p className="actions">
        <button className="link" onClick={onUndo} disabled={!canUndo} title="⌘Z">
          Annuler
        </button>
        <button className="link" onClick={onRedo} disabled={!canRedo} title="⇧⌘Z">
          Rétablir
        </button>
        <span className="actions-sep" aria-hidden="true" />
        {onRecompose && (
          <button
            className="link"
            onClick={onRecompose}
            title="Recompose l'album ; les planches éditées ou verrouillées sont conservées"
          >
            Recomposer
          </button>
        )}
        <button
          className={"link" + (dirty ? " dirty" : "")}
          onClick={onSave}
          disabled={!dirty}
          title="⌘S"
        >
          Enregistrer
        </button>
        {onPdf && (
          <button
            className="link"
            onClick={onPdf}
            disabled={pdfBusy}
            title="Rend le PDF puis l'enregistre où vous voulez"
          >
            {pdfBusy ? "PDF…" : "PDF"}
          </button>
        )}
        <span className="actions-sep" aria-hidden="true" />
        <button className="link" onClick={onClose} title="Fermer et composer un autre album">
          Nouveau
        </button>
        <button className="link" onClick={onOpen} title="⌘O">
          Ouvrir
        </button>
      </p>
    </header>
  );
}

/**
 * The book's foot: page-turn arrows, a ruler graduated one tick per spread
 * (chapter starts marked in accent, their caption on hover, drag a tick to
 * move that spread), the cover tick, the position, and a fixed line that
 * hosts hints, statuses and the caption editor of the selected photo.
 * Constant height, whatever shows: the spread above never moves.
 */
function BookFoot({
  album,
  index,
  total,
  status,
  overflow,
  rendering,
  onCancelExport,
  selectedSlot,
  onCaption,
  onSeek,
  onMoveSpread,
}: {
  album: Album;
  index: number;
  total: number;
  status: string | null;
  overflow: string | null;
  rendering: boolean;
  onCancelExport: () => void;
  selectedSlot: Slot | null;
  onCaption?: (text: string) => void;
  onSeek: (i: number) => void;
  onMoveSpread: (from: number, to: number) => void;
}) {
  const ruler = useRef<HTMLElement>(null);
  const drag = useRef<{ from: number; startX: number; moved: boolean } | null>(null);
  const [dropTick, setDropTick] = useState<number | null>(null);

  const tickAt = (clientX: number): number => {
    const el = ruler.current;
    if (!el || total < 2) return 0;
    const r = el.getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (clientX - r.left) / r.width));
    return Math.round(f * (total - 1));
  };

  return (
    <footer className="foot">
      <div className="foot-nav">
        <button
          className="foot-arrow"
          onClick={() => onSeek(index - 1)}
          disabled={index <= COVER}
          aria-label="Planche précédente"
          title="←"
        >
          ‹
        </button>
        <button
          className={"ruler-cover" + (index === COVER ? " current" : "")}
          onClick={() => onSeek(COVER)}
          title="Couverture"
        >
          C
        </button>
        <nav
          ref={ruler}
          className="ruler"
          aria-label="Aller à une planche, glisser un trait pour déplacer sa planche"
          onPointerMove={(e) => {
            const d = drag.current;
            if (!d) return;
            if (!d.moved && Math.abs(e.clientX - d.startX) < 5) return;
            d.moved = true;
            setDropTick(tickAt(e.clientX));
          }}
          onPointerUp={(e) => {
            const d = drag.current;
            drag.current = null;
            setDropTick(null);
            if (!d) return;
            if (d.moved) {
              const to = tickAt(e.clientX);
              if (to !== d.from) onMoveSpread(d.from, to);
            } else {
              onSeek(d.from);
            }
          }}
          onPointerCancel={() => {
            drag.current = null;
            setDropTick(null);
          }}
        >
          {album.spreads.map((s, i) => (
            <button
              key={i}
              className={
                "ruler-tick" +
                (s.caption ? " chapter" : "") +
                (i === index ? " current" : "") +
                (dropTick === i ? " droptick" : "")
              }
              style={{ left: `${total > 1 ? (i / (total - 1)) * 100 : 0}%` }}
              title={
                (s.caption ? `${s.caption} · ` : "") +
                `planche ${i + 1} · glisser pour la déplacer`
              }
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                drag.current = { from: i, startX: e.clientX, moved: false };
                (e.currentTarget.closest(".ruler") as HTMLElement)?.setPointerCapture(
                  e.pointerId,
                );
              }}
            />
          ))}
          {index >= 0 && (
            <span
              className="ruler-mark"
              style={{ left: `${total > 1 ? (index / (total - 1)) * 100 : 0}%` }}
            />
          )}
        </nav>
        <button
          className="foot-arrow"
          onClick={() => onSeek(index + 1)}
          disabled={index >= total - 1}
          aria-label="Planche suivante"
          title="→ ou espace"
        >
          ›
        </button>
        <span className="foot-pos">
          {index === COVER ? "C" : index + 1} / {total}
        </span>
      </div>
      <div className="foot-line">
        {rendering ? (
          <span className="foot-render">
            {status ?? "Rendu du PDF d’impression…"}{" "}
            <button className="link" onClick={onCancelExport}>
              Annuler l'export
            </button>
          </span>
        ) : selectedSlot && onCaption ? (
          <CaptionEditor slot={selectedSlot} onCaption={onCaption} status={status} />
        ) : (
          <span className={overflow && !status ? "foot-overflow" : undefined}>
            {status ??
              overflow ??
              (index === COVER
                ? "La couverture : titre et sous-titre en place, glissez la photo pour la recadrer."
                : "")}
          </span>
        )}
      </div>
    </footer>
  );
}

/**
 * The caption editor of the selected photo, living in the foot's fixed
 * line: an input, the EXIF suggestion one click away, and the crop hints.
 */
function CaptionEditor({
  slot,
  onCaption,
  status,
}: {
  slot: Slot;
  onCaption: (text: string) => void;
  status: string | null;
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

  return (
    <span className="foot-captioning">
      <label className="foot-caption-label">
        Légende
        <input
          className="foot-caption"
          value={value}
          placeholder="aucune"
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
          title="Date EXIF de la photo, proposée, jamais imposée"
        >
          proposer « {suggestion} »
        </button>
      )}
      <span className="foot-caption-hints">
        {status ??
          "Glisser recadre · molette zoome · double-clic recentre · ⌥ affine · ⌫ retire"}
      </span>
    </span>
  );
}

/** The light table's foot: insertion and spread actions on the current one. */
function PlanchesFoot({
  album,
  index,
  status,
  onInsert,
  onDuplicate,
  onRemove,
}: {
  album: Album;
  index: number;
  status: string | null;
  onInsert: (kind: "vide" | "texte") => void;
  onDuplicate: () => void;
  onRemove: () => void;
}) {
  const spread = album.spreads[index];
  return (
    <footer className="foot">
      <div className="foot-nav planches-actions">
        <span className="foot-pos planches-pos">
          planche {index + 1} / {album.spreads.length}
          {spread?.caption ? ` · ${spread.caption}` : ""}
        </span>
        <button className="link" onClick={() => onInsert("vide")} title="Après la planche courante">
          + planche vide
        </button>
        <button className="link" onClick={() => onInsert("texte")} title="Après la planche courante">
          + planche de texte
        </button>
        <span className="actions-sep" aria-hidden="true" />
        <button className="link" onClick={onDuplicate} title="⌘D">
          Dupliquer
        </button>
        <button className="link" onClick={onRemove} title="⌫">
          Supprimer
        </button>
      </div>
      <p className="foot-line">
        {status ??
          "Glissez une planche sur une autre pour la déplacer. Double-clic ouvre dans le Livre, ⌘L fige."}
      </p>
    </footer>
  );
}

/**
 * The sorting view's foot. Nothing selected: what this view is. A photo
 * selected: its name, the photo that won over it (a click shows its spread),
 * and the rescue. Same fixed height as the book's foot.
 */
function TriFoot({
  entry,
  status,
  onRescue,
  onShowSpread,
}: {
  entry: TriEntry | null;
  status: string | null;
  onRescue: (entry: TriEntry) => void;
  onShowSpread: (src: string) => void;
}) {
  return (
    <footer className="foot">
      {entry ? (
        <div className="foot-tri">
          <code className="foot-tri-name">{entry.src.split("/").pop()}</code>
          {entry.kept && (
            <button
              className="foot-winner"
              onClick={() => onShowSpread(entry.kept!)}
              title="Voir la planche de la photo gardée"
            >
              <MiniThumb src={entry.kept} />
              <span>gardée à sa place · voir la planche</span>
            </button>
          )}
          <button className="cta small" onClick={() => onRescue(entry)}>
            Repêcher
          </button>
        </div>
      ) : (
        <div className="foot-tri muted">
          Photos écartées par la curation ou retirées à la main. Un clic pour
          les détails, un double-clic repêche. Le tiroir du Livre les garde
          aussi à portée de glisser.
        </div>
      )}
      <p className="foot-line">{status ?? ""}</p>
    </footer>
  );
}

/** A postage-stamp thumbnail, for the foot. */
function MiniThumb({ src }: { src: string }) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(src));
  useEffect(() => {
    let alive = true;
    if (!cachedThumb(src)) setUrl(undefined);
    loadThumb(src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [src]);
  return (
    <span className="mini-thumb">{url && <img src={url} alt="" />}</span>
  );
}

function Empty({
  onOpen,
  onCreate,
  building,
  busyTitle,
  error,
  onCancelBuild,
}: {
  onOpen: () => void;
  onCreate: (dir: string, format: string, spreads: number, title: string | null) => void;
  building: string[] | null;
  busyTitle: string | null;
  error: string | null;
  onCancelBuild: () => void;
}) {
  const [formats, setFormats] = useState<FormatPreset[]>([]);
  const [dir, setDir] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [format, setFormat] = useState("carre-21");
  const [spreads, setSpreads] = useState(48);

  useEffect(() => {
    listFormats().then(setFormats, () => {});
  }, []);

  const folderName = dir?.split("/").pop() ?? "";
  const chosen = formats.find((f) => f.name === format);

  const pick = async () => {
    const picked = await pickPhotosFolder();
    if (!picked) return;
    setDir(picked);
    setTitle(picked.split("/").pop() ?? "");
  };

  return (
    <div className="empty">
      <div className={"empty-block" + (dir || building ? " wide" : "")}>
        <p className="kicker">Colophon</p>

        {!dir && !building && (
          <>
            <h1>
              Un dossier de photos,
              <br />
              un album à feuilleter.
            </h1>
            <p className="lede">
              Colophon lit vos photos, écarte les doublons et les ratés,
              compose les planches et rend un PDF prêt à relire. Tout se
              retouche ensuite : gabarits, ordre, photos repêchées.
            </p>
            <button className="cta" onClick={() => void pick()}>
              Choisir un dossier de photos…
            </button>
            <p className="hint">
              ou{" "}
              <button className="link" onClick={onOpen}>
                ouvrir un album existant
              </button>{" "}
              (<kbd>⌘</kbd> <kbd>O</kbd>)
            </p>
          </>
        )}

        {(dir || building) && (
          <div className="setup-layout">
            {dir && !building && (
              <form
                className="setup"
                onSubmit={(e) => {
                  e.preventDefault();
                  onCreate(dir, format, spreads, title.trim() || null);
                }}
              >
                <h1 className="setup-heading">Nouvel album</h1>
                <p className="setup-folder">
                  <code>{dir}</code>
                  <button type="button" className="link" onClick={() => void pick()}>
                    changer de dossier
                  </button>
                </p>

                <label className="setup-field">
                  <span className="setup-label">titre</span>
                  <input
                    className="setup-input"
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    placeholder={folderName}
                    autoFocus
                  />
                </label>

                <div className="setup-field">
                  <span className="setup-label">format de page</span>
                  <div className="format-cards">
                    {formats.map((f) => (
                      <FormatCard
                        key={f.name}
                        f={f}
                        active={f.name === format}
                        onPick={() => setFormat(f.name)}
                      />
                    ))}
                  </div>
                </div>

                <label className="setup-field">
                  <span className="setup-label">planches</span>
                  <span className="setup-spreads">
                    <input
                      className="setup-input narrow"
                      type="number"
                      min={8}
                      max={200}
                      value={spreads}
                      onChange={(e) => setSpreads(Number(e.target.value) || 48)}
                    />
                    <span className="setup-hint">
                      soit {spreads * 2} pages, l'imprimeur compte en pages
                    </span>
                  </span>
                </label>

                <p className="setup-actions">
                  <button className="cta" type="submit">
                    Composer l'album
                  </button>
                  <button type="button" className="link" onClick={() => setDir(null)}>
                    Annuler
                  </button>
                </p>
              </form>
            )}

            {building && (
              <div className="setup">
                <h1 className="setup-heading">
                  {busyTitle
                    ? `Recomposition de « ${busyTitle} »`
                    : `Composition de « ${title.trim() || folderName || "l'album"} »`}
                </h1>
                <BuildProgress lines={building} onCancel={onCancelBuild} />
                <p className="setup-hint">
                  {busyTitle
                    ? "Les planches éditées à la main ou verrouillées sont conservées telles quelles."
                    : "L'analyse des photos ne se fait qu'une fois : recomposer ce dossier sera bien plus rapide."}
                </p>
              </div>
            )}

            {chosen && <FormatSpreadPreview f={chosen} />}
          </div>
        )}

        {error && <p className="warn">{error}</p>}
      </div>
    </div>
  );
}

const cm = (mm: number) =>
  (mm / 10).toLocaleString("fr-FR", { maximumFractionDigits: 1 });

/**
 * One page format, drawn as an open double page at its true proportions:
 * the shapes differ, so the choice is visible before any vocabulary.
 */
function FormatCard({
  f,
  active,
  onPick,
}: {
  f: FormatPreset;
  active: boolean;
  onPick: () => void;
}) {
  const pageH = 44;
  const pageW = (f.w / f.h) * pageH;
  return (
    <button
      type="button"
      className={"format-card" + (active ? " active" : "")}
      onClick={onPick}
      title={f.about}
    >
      <span className="format-preview">
        <span className="format-page" style={{ width: pageW, height: pageH }} />
        <span className="format-page" style={{ width: pageW, height: pageH }} />
      </span>
      <span className="format-name">{f.name.replace(/-/g, " ")}</span>
      <span className="format-dims">
        {cm(f.w)} × {cm(f.h)} cm
      </span>
    </button>
  );
}

/**
 * The chosen format, drawn large with the real spread geometry: the actual
 * margins, gutter and a six-photo template from the engine's own arithmetic,
 * plus its measurements. What you pick is what the press trims.
 */
function FormatSpreadPreview({ f }: { f: FormatPreset }) {
  const album = { trim_mm: { w: f.w, h: f.h }, bleed_mm: 0 } as Album;
  const canvas = mediaCanvas(album);
  const rects = slotsFor("six", 6, canvas);
  const width = 320;
  const scale = width / canvas.w;

  return (
    <figure className="format-large">
      <span className="format-large-cote">{cm(f.w * 2)} cm ouvert</span>
      <div
        className="format-large-spread"
        style={{ width: canvas.w * scale, height: canvas.h * scale }}
      >
        {rects.map((r, i) => (
          <span
            key={i}
            className="format-large-slot"
            style={{
              left: r.x * scale,
              top: r.y * scale,
              width: r.w * scale,
              height: r.h * scale,
            }}
          />
        ))}
        <span className="format-large-fold" />
      </div>
      <figcaption>
        <strong>
          {cm(f.w)} × {cm(f.h)} cm
        </strong>{" "}
        la page · {f.about}
      </figcaption>
    </figure>
  );
}

/**
 * The engine's progress lines, read into a bar and a stage label. The
 * analysis phase streams counts, so the bar moves all along the longest
 * stretch instead of freezing at « analyse ».
 */
function buildStage(lines: string[]): { pct: number; label: string } {
  let pct = 2;
  let label = "lecture du dossier";
  for (const l of lines) {
    let p = 0;
    let lab = "";
    const count = l.match(/^analyze: (\d+)\/(\d+)/);
    if (l.startsWith("scan:")) {
      p = 4;
      lab = "inventaire du dossier";
    } else if (count) {
      const [, i, n] = count;
      p = 5 + (65 * Number(i)) / Math.max(1, Number(n));
      lab = `analyse des photos, ${i} sur ${n}`;
    } else if (l.startsWith("analyze:")) {
      p = 70;
      lab = "analyse des photos";
    } else if (l.startsWith("junk:") || l.startsWith("note:")) {
      p = 72;
      lab = "écart des parasites";
    } else if (l.startsWith("dedup:")) {
      p = 76;
      lab = "déduplication des rafales";
    } else if (l.startsWith("thinning:")) {
      p = 80;
      lab = "éclaircissage des doublons";
    } else if (l.startsWith("chapters:")) {
      p = 84;
      lab = "découpage en chapitres";
    } else if (l.startsWith("layout:")) {
      p = 88;
      lab = "mise en page des planches";
    } else if (l.startsWith("pinned:")) {
      p = 90;
      lab = "planches éditées remises en place";
    } else if (l.startsWith("curation:")) {
      p = 92;
      lab = "journal de curation";
    } else if (l.startsWith("pdf:")) {
      p = 96;
      lab = "rendu du PDF";
    }
    if (p >= pct) {
      pct = p;
      label = lab;
    }
  }
  return { pct, label };
}

function BuildProgress({
  lines,
  onCancel,
}: {
  lines: string[];
  onCancel: () => void;
}) {
  const { pct, label } = buildStage(lines);
  const log = lines.filter((l) => !/^analyze: \d+\/\d+$/.test(l)).slice(-6);
  return (
    <div className="build">
      <div
        className="build-bar"
        role="progressbar"
        aria-valuenow={Math.round(pct)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <span style={{ width: `${pct}%` }} />
      </div>
      <p className="build-stage">
        <span key={label} className="build-stage-label">
          {label}
        </span>
        <span className="build-actions">
          <button
            className="link"
            type="button"
            onClick={onCancel}
            title="Arrête la composition ; rien n'est écrit"
          >
            Annuler
          </button>
          <span className="build-pct">{Math.round(pct)} %</span>
        </span>
      </p>
      <pre className="buildlog">
        {log.length ? log.join("\n") : "lecture du dossier…"}
      </pre>
    </div>
  );
}
