import { useCallback, useEffect, useRef, useState } from "react";
import {
  buildAlbum,
  exportPdf,
  fetchCuration,
  FormatPreset,
  inTauri,
  listFormats,
  onBuildProgress,
  openAlbum as openAlbumAt,
  pickAlbumFolder,
  pickPhotosFolder,
  saveAlbum,
} from "./bridge";
import {
  Album,
  Discard,
  mediaCanvas,
  OpenedAlbum,
  slotsFor,
  Spread,
} from "./album";
import {
  changeTemplate,
  moveBlocker,
  movePhoto,
  removePhoto,
  rescuePhoto,
  spreadOf,
  swapPhotos,
  triEntries,
  TriEntry,
} from "./edits";
import { SpreadView } from "./SpreadView";
import { TemplatePicker } from "./TemplatePicker";
import { TriView } from "./TriView";
import { cachedThumb, loadThumb, resetThumbs } from "./thumbs";
import "./styles.css";

/** Full album snapshots: a 50-spread album is a few tens of kilobytes. */
type History = { album: Album; past: Album[]; future: Album[] };
const HISTORY_CAP = 50;

export default function App() {
  const [opened, setOpened] = useState<OpenedAlbum | null>(null);
  const [hist, setHist] = useState<History | null>(null);
  const [savedAlbum, setSavedAlbum] = useState<Album | null>(null);
  const [index, setIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [building, setBuilding] = useState<string[] | null>(null);
  const [rendering, setRendering] = useState(false);
  const [view, setView] = useState<"livre" | "tri">("livre");
  const [curation, setCuration] = useState<Discard[]>([]);
  const [triSelected, setTriSelected] = useState<string | null>(null);

  const album = hist?.album ?? null;
  const total = album?.spreads.length ?? 0;
  const dirty = album !== null && album !== savedAlbum;

  const openAlbum = useCallback(async () => {
    const picked = await pickAlbumFolder();
    if (picked === null) return;
    try {
      const result = await openAlbumAt(picked);
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
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      setError(String(e));
    }
  }, []);

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

  const save = useCallback(async () => {
    if (!hist) return false;
    try {
      await saveAlbum(hist.album);
      setSavedAlbum(hist.album);
      setStatus("Enregistré");
      return true;
    } catch (e) {
      setStatus(String(e));
      return false;
    }
  }, [hist]);

  const regenPdf = useCallback(async () => {
    if (rendering || !hist || !(await save())) return;
    setRendering(true);
    setStatus("Rendu du PDF d’impression…");
    try {
      const dest = await exportPdf(hist.album.title, (done, total) =>
        setStatus(`Rendu à 300 dpi : ${done}/${total} photos…`),
      );
      setStatus(dest ? `PDF enregistré : ${dest}` : "Enregistrement annulé");
    } catch (e) {
      setStatus(String(e));
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
    setError(null);
    // Every line is kept: the counter lines drive the progress bar, the
    // named stages feed the visible log.
    const off = await onBuildProgress((line) =>
      setBuilding((b) => (b ? [...b, line] : [line])),
    );
    try {
      const result = await buildAlbum(dir, format, spreads, title);
      resetThumbs();
      setOpened(result);
      setHist({ album: result.album, past: [], future: [] });
      setSavedAlbum(result.album);
      setIndex(0);
      setSelected(null);
      setTriSelected(null);
      setView("livre");
      setStatus(null);
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      setError(String(e));
    } finally {
      off();
      setBuilding(null);
    }
  }, []);

  /** Back to the creation screen. Unsaved work asks before dying. */
  const closeAlbum = useCallback(() => {
    if (dirty && !window.confirm("Des modifications ne sont pas enregistrées. Fermer quand même ?")) {
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
      const anchor = anchorByWinner >= 0 ? anchorByWinner : index;
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
      // button takes space and enter (standard activation).
      const t = e.target as HTMLElement | null;
      if (t && /^(INPUT|SELECT|TEXTAREA|BUTTON)$/.test(t.tagName)) return;

      const key = e.key.toLowerCase();
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
      if (e.metaKey && (key === "1" || key === "2")) {
        e.preventDefault();
        setView(key === "1" ? "livre" : "tri");
        setSelected(null);
        setTriSelected(null);
        return;
      }
      // The sorting view keeps the global shortcuts, nothing spread-bound.
      if (view === "tri") {
        if (e.key === "Escape") setTriSelected(null);
        return;
      }
      if (!total) return;
      if (
        e.metaKey &&
        e.shiftKey &&
        (e.key === "ArrowRight" || e.key === "ArrowLeft") &&
        selected !== null &&
        album
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
      if ((e.key === "Backspace" || e.key === "Delete") && selected !== null) {
        e.preventDefault();
        setSelected(null);
        apply((a) => removePhoto(a, index, selected));
        return;
      }
      if (e.key === "Escape") {
        setSelected(null);
        return;
      }
      const step = (d: number) =>
        setIndex((i) => Math.min(total - 1, Math.max(0, i + d)));
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

  if (!album || total === 0) {
    return (
      <Empty
        onOpen={openAlbum}
        onCreate={createAlbum}
        building={building}
        error={error}
      />
    );
  }

  const spread = album.spreads[Math.min(index, total - 1)];
  const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
  const triEntry = entries.find((e) => e.src === triSelected) ?? null;

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
        }}
        onTemplate={(t) => apply((a) => changeTemplate(a, index, t))}
        onUndo={undo}
        onRedo={redo}
        onSave={() => void save()}
        onPdf={inTauri ? () => void regenPdf() : undefined}
        pdfBusy={rendering}
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
      ) : (
        <main className="stage">
          <div className="turn" key={index}>
            <SpreadView
              album={album}
              spread={spread}
              selected={selected}
              onSelect={setSelected}
              onSwap={(a, b) => apply((al) => swapPhotos(al, index, a, b))}
            />
          </div>
        </main>
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
      ) : (
        <BookFoot
          album={album}
          index={index}
          total={total}
          status={status}
          selected={selected !== null}
          onSeek={(i) => setIndex(Math.min(total - 1, Math.max(0, i)))}
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
  onUndo,
  onRedo,
  onSave,
  onPdf,
  pdfBusy,
  onOpen,
  onClose,
}: {
  album: Album;
  spread: Spread;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  view: "livre" | "tri";
  triCount: number;
  onView: (v: "livre" | "tri") => void;
  onTemplate: (t: string) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onPdf?: () => void;
  pdfBusy?: boolean;
  onOpen: () => void;
  onClose: () => void;
}) {
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
        </span>
        {view === "livre" && (
          <TemplatePicker album={album} spread={spread} onPick={onTemplate} />
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
 * (chapter starts marked in accent, their caption on hover), the position,
 * and a fixed line for hints and statuses. Constant height, whatever shows:
 * the spread above never moves.
 */
function BookFoot({
  album,
  index,
  total,
  status,
  selected,
  onSeek,
}: {
  album: Album;
  index: number;
  total: number;
  status: string | null;
  selected: boolean;
  onSeek: (i: number) => void;
}) {
  return (
    <footer className="foot">
      <div className="foot-nav">
        <button
          className="foot-arrow"
          onClick={() => onSeek(index - 1)}
          disabled={index === 0}
          aria-label="Planche précédente"
          title="←"
        >
          ‹
        </button>
        <nav className="ruler" aria-label="Aller à une planche">
          {album.spreads.map((s, i) => (
            <button
              key={i}
              className={
                "ruler-tick" +
                (s.caption ? " chapter" : "") +
                (i === index ? " current" : "")
              }
              style={{ left: `${total > 1 ? (i / (total - 1)) * 100 : 0}%` }}
              title={s.caption ? `${s.caption} · planche ${i + 1}` : `planche ${i + 1}`}
              onClick={() => onSeek(i)}
            />
          ))}
          <span
            className="ruler-mark"
            style={{ left: `${total > 1 ? (index / (total - 1)) * 100 : 0}%` }}
          />
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
          {index + 1} / {total}
        </span>
      </div>
      <p className="foot-line">
        {status ??
          (selected ? (
            <>
              <kbd>⌫</kbd> retire la photo, le gabarit suit. Glissez une photo
              sur une autre pour les permuter, <kbd>⌘⇧←</kbd> <kbd>⌘⇧→</kbd>{" "}
              pour l'envoyer sur la planche voisine.
            </>
          ) : (
            ""
          ))}
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
          les détails, un double-clic repêche.
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
  error,
}: {
  onOpen: () => void;
  onCreate: (dir: string, format: string, spreads: number, title: string | null) => void;
  building: string[] | null;
  error: string | null;
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
                  Composition de « {title.trim() || folderName || "l'album"} »
                </h1>
                <BuildProgress lines={building} />
                <p className="setup-hint">
                  L'analyse des photos ne se fait qu'une fois : recomposer ce
                  dossier sera bien plus rapide.
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

function BuildProgress({ lines }: { lines: string[] }) {
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
        <span className="build-pct">{Math.round(pct)} %</span>
      </p>
      <pre className="buildlog">
        {log.length ? log.join("\n") : "lecture du dossier…"}
      </pre>
    </div>
  );
}
