import { useCallback, useEffect, useState } from "react";
import { inTauri, openAlbum as openAlbumAt, pickAlbumFolder } from "./bridge";
import { Album, OpenedAlbum } from "./album";
import { SpreadView } from "./SpreadView";
import { loadThumb, resetThumbs } from "./thumbs";
import "./styles.css";

export default function App() {
  const [opened, setOpened] = useState<OpenedAlbum | null>(null);
  const [index, setIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const openAlbum = useCallback(async () => {
    const picked = await pickAlbumFolder();
    if (picked === null) return;
    try {
      const result = await openAlbumAt(picked);
      resetThumbs();
      setOpened(result);
      setIndex(0);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const album = opened?.album ?? null;
  const total = album?.spreads.length ?? 0;

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
      if (e.metaKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        void openAlbum();
        return;
      }
      if (!total) return;
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
  }, [total, openAlbum]);

  if (!album) return <Empty onOpen={openAlbum} error={error} />;

  return (
    <div className="app">
      <Bar album={album} index={index} total={total} onOpen={openAlbum} />
      <main className="stage">
        <div className="turn" key={index}>
          <SpreadView album={album} spread={album.spreads[index]} />
        </div>
      </main>
      <Progress index={index} total={total} onSeek={setIndex} />
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
  index,
  total,
  onOpen,
}: {
  album: Album;
  index: number;
  total: number;
  onOpen: () => void;
}) {
  const spread = album.spreads[index];
  return (
    <header className="bar">
      <h1>{album.title}</h1>
      <p className="meta">
        <span>
          planche {index + 1} sur {total}
        </span>
        <span className="template">{spread.template}</span>
      </p>
      <button className="link" onClick={onOpen}>
        Ouvrir
      </button>
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
