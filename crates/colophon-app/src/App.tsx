import { useCallback, useEffect, useState } from "react";
import {
  inTauri,
  openAlbum as openAlbumAt,
  pickAlbumFolder,
  renderPdf,
  saveAlbum,
} from "./bridge";
import { Album, OpenedAlbum, Spread } from "./album";
import { changeTemplate, removePhoto, swapPhotos, templateChoices } from "./edits";
import { SpreadView } from "./SpreadView";
import { loadThumb, resetThumbs } from "./thumbs";
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
      setError(null);
      setStatus(null);
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
    if (!(await save())) return;
    setStatus("Rendu du PDF…");
    try {
      const path = await renderPdf();
      setStatus(`PDF régénéré : ${path.split("/").pop()}`);
    } catch (e) {
      setStatus(String(e));
    }
  }, [save]);

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
  // away, there is nothing to pick.
  useEffect(() => {
    if (!inTauri && !opened) void openAlbum();
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
      // A focused select or input owns the keyboard.
      const t = e.target as HTMLElement | null;
      if (t && /^(INPUT|SELECT|TEXTAREA)$/.test(t.tagName)) return;

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
      if (!total) return;
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
  }, [total, openAlbum, save, undo, redo, apply, index, selected]);

  if (!album || total === 0) return <Empty onOpen={openAlbum} error={error} />;

  const spread = album.spreads[Math.min(index, total - 1)];

  return (
    <div className="app">
      <Bar
        album={album}
        spread={spread}
        index={index}
        total={total}
        dirty={dirty}
        canUndo={(hist?.past.length ?? 0) > 0}
        canRedo={(hist?.future.length ?? 0) > 0}
        status={status}
        onTemplate={(t) => apply((a) => changeTemplate(a, index, t))}
        onUndo={undo}
        onRedo={redo}
        onSave={() => void save()}
        onPdf={inTauri ? () => void regenPdf() : undefined}
        onOpen={openAlbum}
      />
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
      <Progress index={index} total={total} onSeek={setIndex} />
      {selected !== null && (
        <p className="hintbar">
          <kbd>⌫</kbd> retire la photo, le gabarit suit. Glissez une photo sur
          une autre pour les permuter.
        </p>
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
  index,
  total,
  dirty,
  canUndo,
  canRedo,
  status,
  onTemplate,
  onUndo,
  onRedo,
  onSave,
  onPdf,
  onOpen,
}: {
  album: Album;
  spread: Spread;
  index: number;
  total: number;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  status: string | null;
  onTemplate: (t: string) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onPdf?: () => void;
  onOpen: () => void;
}) {
  return (
    <header className="bar">
      <h1>{album.title}</h1>
      <p className="meta">
        <span>
          planche {index + 1} sur {total}
        </span>
        <select
          className="template-pick"
          value={spread.template}
          onChange={(e) => {
            onTemplate(e.target.value);
            e.target.blur();
          }}
          title="Gabarit de la planche"
        >
          {templateChoices(spread).map(([t, cap]) => (
            <option key={t} value={t}>
              {t} · {cap}
            </option>
          ))}
        </select>
        {status && <span className="status">{status}</span>}
      </p>
      <p className="actions">
        <button className="link" onClick={onUndo} disabled={!canUndo} title="⌘Z">
          Annuler
        </button>
        <button className="link" onClick={onRedo} disabled={!canRedo} title="⇧⌘Z">
          Rétablir
        </button>
        <button
          className={"link" + (dirty ? " dirty" : "")}
          onClick={onSave}
          disabled={!dirty}
          title="⌘S"
        >
          Enregistrer
        </button>
        {onPdf && (
          <button className="link" onClick={onPdf} title="Régénère album.pdf">
            PDF
          </button>
        )}
        <button className="link" onClick={onOpen}>
          Ouvrir
        </button>
      </p>
    </header>
  );
}

function Progress({
  index,
  total,
  onSeek,
}: {
  index: number;
  total: number;
  onSeek: (i: number) => void;
}) {
  return (
    <nav
      className="progress"
      onClick={(e) => {
        const box = e.currentTarget.getBoundingClientRect();
        const ratio = (e.clientX - box.left) / box.width;
        onSeek(Math.min(total - 1, Math.max(0, Math.round(ratio * (total - 1)))));
      }}
    >
      <span
        className="progress-mark"
        style={{ left: `${total > 1 ? (index / (total - 1)) * 100 : 0}%` }}
      />
    </nav>
  );
}

function Empty({
  onOpen,
  error,
}: {
  onOpen: () => void;
  error: string | null;
}) {
  return (
    <div className="empty">
      <div className="empty-block">
        <p className="kicker">Colophon</p>
        <h1>
          Un dossier de photos,
          <br />
          un album à feuilleter.
        </h1>
        <p className="lede">
          Ouvrez un dossier produit par la ligne de commande, celui qui contient
          <code> album.json</code>. La vue Livre affiche les planches telles
          qu'elles seront imprimées.
        </p>
        <button className="cta" onClick={onOpen}>
          Ouvrir un album
        </button>
        <p className="hint">
          ou <kbd>⌘</kbd> <kbd>O</kbd>, puis les flèches pour tourner les pages
        </p>
        {error && <p className="warn">{error}</p>}
      </div>
    </div>
  );
}
