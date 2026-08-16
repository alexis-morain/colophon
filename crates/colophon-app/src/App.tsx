import { useCallback, useEffect, useRef, useState } from "react";
import {
  BuildBilan,
  buildAlbum,
  cancelBuild,
  cancelExport,
  confirmDialog,
  exportPdf,
  fetchCuration,
  FormatPreset,
  inTauri,
  listDensities,
  DensitePreset,
  listFormats,
  listPrinters,
  onBuildProgress,
  openAlbum as openAlbumAt,
  pickAlbumFolder,
  pickPhotosFolder,
  Printer,
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
import { BilanView } from "./BilanView";
import { SpreadView } from "./SpreadView";
import { TemplatePicker } from "./TemplatePicker";
import { RevueView, TriView } from "./TriView";
import { Drawer } from "./Drawer";
import { PlanchesView, LockGlyph } from "./PlanchesView";
import { CoverView } from "./CoverView";
import { EnvoiView } from "./EnvoiView";
import { RaccourcisView } from "./Raccourcis";
import { SignalerView } from "./SignalerView";
import { SignalKind } from "./signaler";
import { Chevron, CoverGlyph } from "./icons";
import { installMenu, MenuActions, RecentAlbum } from "./menu";
import { readRecents, pushRecent } from "./recents";
import { cachedThumb, loadThumb, resetThumbs } from "./thumbs";
import "./styles.css";

/** Full album snapshots: a 50-spread album is a few tens of kilobytes. */
type History = { album: Album; past: Album[]; future: Album[] };
const HISTORY_CAP = 50;

/** An error told to the user: a French sentence first, the raw detail behind
 *  a disclosure. Raw `String(e)` never reaches the screen on its own. */
type Fault = { quoi: string; detail: string };

const fault = (quoi: string, e: unknown): Fault => ({
  quoi,
  detail: String(e),
});

/** The banner a Fault renders as: the sentence, the detail folded away. */
function FaultBlock({
  fault,
  onDismiss,
}: {
  fault: Fault;
  onDismiss?: () => void;
}) {
  return (
    <div className="warn fault">
      <p className="fault-quoi">
        {fault.quoi}
        {onDismiss && (
          <button className="link" onClick={onDismiss}>
            Fermer
          </button>
        )}
      </p>
      <details className="fault-detail">
        <summary>Détail technique</summary>
        <pre>{fault.detail}</pre>
      </details>
    </div>
  );
}

type View = "livre" | "tri" | "planches" | "envoi";

/** In the book view, index -1 is the cover. */
const COVER = -1;

export default function App() {
  const [opened, setOpened] = useState<OpenedAlbum | null>(null);
  const [hist, setHist] = useState<History | null>(null);
  const [savedAlbum, setSavedAlbum] = useState<Album | null>(null);
  const [index, setIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<Fault | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [building, setBuilding] = useState<string[] | null>(null);
  // The end-of-build report; the book opens behind it, it dismisses once.
  const [bilan, setBilan] = useState<BuildBilan | null>(null);
  // Index into the sorting view's entries while the keyboard review is up.
  const [revue, setRevue] = useState<number | null>(null);
  const [busyTitle, setBusyTitle] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  const [view, setView] = useState<View>("livre");
  const [curation, setCuration] = useState<Discard[]>([]);
  const [triSelected, setTriSelected] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [overflow, setOverflow] = useState<string | null>(null);
  // The supplier the destination screen reads. One per session:
  // the album carries no printer, the file it produces does.
  const [profil, setProfil] = useState("cloudprinter");
  // Loaded once and shared: the cover editor draws its sheet for the same
  // supplier the destination screen preflights against.
  const [printers, setPrinters] = useState<Printer[] | null>(null);
  // The keyboard cheat-sheet overlay (⌘/, menu Aide).
  const [shortcuts, setShortcuts] = useState(false);
  // The report panel (Aide → Signaler), one of the three issue variants.
  const [signaler, setSignaler] = useState<SignalKind | null>(null);
  // The recent-albums list: welcome screen and Fichier menu read it.
  const [recents, setRecents] = useState<RecentAlbum[]>(readRecents);

  useEffect(() => {
    listPrinters().then(setPrinters, () => setPrinters([]));
  }, []);

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
    setBilan(null);
    // The browser harness's « __dev__ » album never enters the list.
    if (inTauri && result.dir) {
      setRecents(pushRecent({ dir: result.dir, title: result.album.title }));
    }
  }, []);

  /** Open one of the recent albums, by its remembered path. */
  const openRecent = useCallback(
    async (dir: string) => {
      try {
        const result = await openAlbumAt(dir);
        adopt(result);
        setCuration(await fetchCuration().catch(() => []));
      } catch (e) {
        setError(
          fault("Cet album n’a pas pu être rouvert. A-t-il été déplacé ?", e),
        );
      }
    },
    [adopt],
  );

  const openAlbum = useCallback(async () => {
    const picked = await pickAlbumFolder();
    if (picked === null) return;
    try {
      const result = await openAlbumAt(picked);
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      setError(fault("L’album n’a pas pu être ouvert.", e));
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
      setError(fault("L’enregistrement a échoué : rien n’a été écrit.", e));
      return false;
    }
  }, []);

  const regenPdf = useCallback(async () => {
    if (rendering || !hist || !(await save())) return;
    setRendering(true);
    setStatus("Rendu du PDF d’impression…");
    try {
      const written = await exportPdf(hist.album.title, profil, (done, total) =>
        setStatus(`Rendu à 300 dpi : ${done}/${total} photos…`),
      );
      // What was actually written, named. A supplier who wants two files gets
      // two, and the second one is the thing nobody thinks to look for.
      setStatus(
        written === null
          ? "Enregistrement annulé"
          : written.length > 1
            ? `${written.length} fichiers enregistrés : ${written.join(" · ")}`
            : `PDF enregistré : ${written[0]}`,
      );
    } catch (e) {
      if (String(e).includes("export annulé")) {
        setStatus("Export annulé, aucun fichier écrit");
      } else {
        setError(fault("Le rendu du PDF a échoué.", e));
      }
    } finally {
      setRendering(false);
    }
  }, [save, rendering, hist, profil]);

  /** Build an album from a photo folder, streaming the engine's progress. */
  const createAlbum = useCallback(async (
    dir: string,
    format: string,
    spreads: number,
    densite: string,
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
      const result = await buildAlbum(dir, format, spreads, densite, title);
      adopt(result.opened);
      setCuration(await fetchCuration().catch(() => []));
      setBilan(result.bilan);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("annulée")) setStatus("Composition annulée");
      else if (msg.includes("aucune photo exploitable"))
        // The engine refused rather than open an empty album. The gesture
        // that gets the user out is choosing another folder, so say so.
        setError(
          fault(
            "Ce dossier n’a donné aucune photo exploitable, rien n’a été créé. " +
              "Choisissez un autre dossier, ou rouvrez celui-ci après y avoir " +
              "ajouté des photos.",
            e,
          ),
        );
      else setError(fault("La composition a échoué.", e));
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
        "Recomposer l’album ? Les planches éditées à la main ou verrouillées " +
          "sont conservées telles quelles, les autres sont recomposées. " +
          "L’historique d’annulation repart de zéro.",
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
      else setError(fault("La recomposition a échoué.", e));
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
          ? "Photo placée · l’ancienne repart dans la réserve"
          : "Photo placée",
      );
    },
    [album, index, apply],
  );

  /** Change view the one canonical way: selections drop, and only the book
   *  view knows the cover. */
  const gotoView = useCallback(
    (v: View) => {
      setView(v);
      setSelected(null);
      setTriSelected(null);
      if (v !== "livre" && index === COVER) setIndex(0);
      // The clicked tab keeps focus and would eat the view's own keys,
      // Enter first among them: give the keyboard back to the view.
      (document.activeElement as HTMLElement | null)?.blur?.();
    },
    [index],
  );

  /** True when the keyboard focus sits in a text field: the field owns its
   *  own undo history and its own editing keys. */
  const inField = () => {
    const el = document.activeElement;
    return el !== null && /^(INPUT|TEXTAREA)$/.test(el.tagName);
  };

  // Every command of the app, by name. The window's keydown handler and the
  // native menu both land here, so a chord and its menu item are one code
  // path with one guard each.
  const raw: Record<string, () => void> = {
    nouveau: () => void closeAlbum(),
    ouvrir: () => void openAlbum(),
    enregistrer: () => {
      // Blur commits the field being edited; save once that landed.
      const el = document.activeElement as HTMLElement | null;
      if (el && inField()) {
        el.blur();
        setTimeout(() => void save(), 0);
      } else {
        void save();
      }
    },
    exporter: () => {
      if (album) gotoView("envoi");
    },
    fermerAlbum: () => void closeAlbum(),
    annuler: () => {
      if (inField()) document.execCommand("undo");
      else undo();
    },
    retablir: () => {
      if (inField()) document.execCommand("redo");
      else redo();
    },
    "vue-livre": () => album && gotoView("livre"),
    "vue-tri": () => album && gotoView("tri"),
    "vue-planches": () => album && gotoView("planches"),
    "vue-envoi": () => album && gotoView("envoi"),
    couverture: () => {
      if (!album) return;
      setView("livre");
      setSelected(null);
      setTriSelected(null);
      setIndex(COVER);
    },
    revue: () => {
      if (!album) return;
      const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
      if (!entries.length) return;
      setView("tri");
      setStatus(null);
      setRevue(0);
    },
    reserve: () => {
      if (!album) return;
      setView("livre");
      setDrawerOpen((o) => !o);
    },
    gabarit: () => {
      if (album && view === "livre" && index >= 0) {
        window.dispatchEvent(new Event("colophon:gabarit"));
      }
    },
    dupliquer: () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      apply((a) => duplicateSpread(a, index));
      setIndex(index + 1);
      setStatus(`Planche ${index + 1} dupliquée`);
    },
    figer: () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      const was = album.spreads[index]?.locked;
      apply((a) => toggleLock(a, index));
      setStatus(
        was
          ? "Planche libérée"
          : "Planche figée : elle survivra à toute recomposition",
      );
    },
    "inserer-vide": () => {
      if (!album || view !== "livre" && view !== "planches") return;
      const at = Math.max(index, 0);
      apply((a) => insertSpread(a, at, "vide"));
      setIndex(at + 1);
      setStatus("Planche vide insérée : une respiration");
    },
    "inserer-texte": () => {
      if (!album || view !== "livre" && view !== "planches") return;
      const at = Math.max(index, 0);
      apply((a) => insertSpread(a, at, "texte"));
      setIndex(at + 1);
      setStatus("Planche de texte insérée : double-clic pour l’ouvrir et écrire");
    },
    "supprimer-planche": () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      apply((a) => removeSpread(a, index));
      setStatus(`Planche ${index + 1} supprimée (⌘Z la ramène)`);
    },
    raccourcis: () => setShortcuts((s) => !s),
    // The three report variants. A bug needs nothing; the two layout
    // complaints quote the spread on screen, the crop one its selected cell.
    "signaler-bug": () => setSignaler("bug"),
    "signaler-planche": () => {
      if (!album || index < 0) {
        setStatus("Ouvrez d’abord la planche à signaler (vue Livre ou Planches)");
        return;
      }
      setSignaler("planche");
    },
    "signaler-recadrage": () => {
      if (!album || index < 0 || selected === null) {
        setStatus("Sélectionnez d’abord la case au recadrage raté (vue Livre)");
        return;
      }
      setSignaler("recadrage");
    },
  };
  const rawRef = useRef(raw);
  rawRef.current = raw;

  // A chord can reach the app twice, once through the window's keydown and
  // once through the menu accelerator, depending on how WebKit routes key
  // equivalents. Whoever speaks second within the window is the same
  // keypress: one action runs.
  const lastFire = useRef({ action: "", source: "", t: 0 });
  const fire = useCallback((source: "kbd" | "menu", action: string) => {
    const now = performance.now();
    const l = lastFire.current;
    if (l.action === action && l.source !== source && now - l.t < 150) return;
    lastFire.current = { action, source, t: now };
    rawRef.current[action]?.();
  }, []);

  // The browser harness has no native menu: a window event stands in for
  // the three Aide → Signaler items, the way the gabarit picker is asked
  // for. Same table, same guards.
  useEffect(() => {
    const onSignal = (e: Event) => {
      const kind = (e as CustomEvent<string>).detail;
      if (kind === "bug" || kind === "planche" || kind === "recadrage") {
        fire("menu", `signaler-${kind}`);
      }
    };
    window.addEventListener("colophon:signaler", onSignal);
    return () => window.removeEventListener("colophon:signaler", onSignal);
  }, [fire]);

  // The native menu follows the app state: rebuilt when the album opens or
  // closes and when the recents change, cheap both times.
  const openRecentRef = useRef(openRecent);
  openRecentRef.current = openRecent;
  const albumOpen = album !== null;
  useEffect(() => {
    if (!inTauri) return;
    const actions: MenuActions = {
      nouveau: () => fire("menu", "nouveau"),
      ouvrir: () => fire("menu", "ouvrir"),
      ouvrirRecent: (dir) => void openRecentRef.current(dir),
      enregistrer: () => fire("menu", "enregistrer"),
      exporter: () => fire("menu", "exporter"),
      fermerAlbum: () => fire("menu", "fermerAlbum"),
      annuler: () => fire("menu", "annuler"),
      retablir: () => fire("menu", "retablir"),
      vue: (v) => fire("menu", `vue-${v}`),
      couverture: () => fire("menu", "couverture"),
      revue: () => fire("menu", "revue"),
      reserve: () => fire("menu", "reserve"),
      gabarit: () => fire("menu", "gabarit"),
      dupliquer: () => fire("menu", "dupliquer"),
      figer: () => fire("menu", "figer"),
      insererVide: () => fire("menu", "inserer-vide"),
      insererTexte: () => fire("menu", "inserer-texte"),
      supprimerPlanche: () => fire("menu", "supprimer-planche"),
      raccourcis: () => fire("menu", "raccourcis"),
      signalerBug: () => fire("menu", "signaler-bug"),
      signalerPlanche: () => fire("menu", "signaler-planche"),
      signalerRecadrage: () => fire("menu", "signaler-recadrage"),
    };
    installMenu(() => actions, albumOpen, recents).catch(() => {
      // A shell without the menu permission still has the window shortcuts.
    });
  }, [albumOpen, recents, fire]);

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

  // Transient status line: every message expires the same way; errors have
  // their own banner and never travel through here.
  useEffect(() => {
    if (!status) return;
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
      // The end-of-build report holds the keyboard: Enter or Escape opens
      // the book, nothing may reach the editor behind it. Enter on a focused
      // button stays native, both its actions dismiss the report anyway.
      if (bilan) {
        if (
          e.key === "Escape" ||
          (e.key === "Enter" && !(t && t.tagName === "BUTTON"))
        ) {
          e.preventDefault();
          setBilan(null);
        }
        return;
      }
      // The keyboard review owns the plain keys, even from a focused button:
      // arrows browse, R rescues, X confirms the discard and moves on,
      // Escape leaves. ⌘-chords keep their app meaning below.
      if (view === "tri" && revue !== null && !e.metaKey && album) {
        const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
        const last = entries.length - 1;
        const i = Math.max(0, Math.min(revue, last));
        if (e.key === "Escape") {
          e.preventDefault();
          setRevue(null);
          return;
        }
        if (e.key === "ArrowLeft") {
          e.preventDefault();
          setStatus(null);
          setRevue(Math.max(0, i - 1));
          return;
        }
        if (e.key === "ArrowRight" || key === "x") {
          e.preventDefault();
          if (i >= last) {
            setRevue(null);
            setStatus("Revue terminée, chaque écart est vu");
          } else {
            setStatus(null);
            setRevue(i + 1);
          }
          return;
        }
        if (key === "r") {
          e.preventDefault();
          if (entries[i]) rescue(entries[i]);
          return;
        }
        return;
      }
      // The report panel holds the keyboard the way the cheat-sheet does;
      // its focused controls keep their native keys.
      if (signaler) {
        if (e.key === "Escape") {
          e.preventDefault();
          setSignaler(null);
        }
        return;
      }
      // The shortcuts overlay holds the keyboard until it closes.
      if (shortcuts) {
        if (e.key === "Escape" || (e.metaKey && key === "/")) {
          e.preventDefault();
          setShortcuts(false);
        }
        return;
      }
      if (t && /^(INPUT|SELECT|TEXTAREA|BUTTON)$/.test(t.tagName)) {
        // A focused field keeps its letters and its own editing chords,
        // native ⌘Z included; a few app chords still pass.
        const appChord =
          e.metaKey && ["s", "o", "1", "2", "3", "4"].includes(key);
        if (!appChord) return;
        if (t.tagName === "BUTTON") t.blur();
      }
      // App chords: one name each, the same names the menu speaks, so a
      // keypress and its menu item are one code path.
      if (e.metaKey) {
        const chord =
          key === "n"
            ? "nouveau"
            : key === "o"
              ? "ouvrir"
              : key === "s"
                ? "enregistrer"
                : key === "z"
                  ? e.shiftKey
                    ? "retablir"
                    : "annuler"
                  : key === "e" && e.shiftKey
                    ? "exporter"
                    : key === "d"
                      ? "dupliquer"
                      : key === "l"
                        ? "figer"
                        : key === "/"
                          ? "raccourcis"
                          : key === "1"
                            ? "vue-livre"
                            : key === "2"
                              ? "vue-tri"
                              : key === "3"
                                ? "vue-planches"
                                : key === "4"
                                  ? "vue-envoi"
                                  : null;
        if (chord) {
          e.preventDefault();
          fire("kbd", chord);
          return;
        }
      }
      // The destination screen has no spread under the cursor: it keeps the
      // global shortcuts and nothing else.
      if (view === "envoi") return;
      // The sorting view keeps the global shortcuts, plus Enter to start
      // the keyboard review; nothing spread-bound.
      if (view === "tri") {
        if (e.key === "Enter" && album) {
          e.preventDefault();
          const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
          if (entries.length) {
            const at = triSelected
              ? entries.findIndex((x) => x.src === triSelected)
              : 0;
            setStatus(null);
            setRevue(Math.max(0, at));
          }
          return;
        }
        if (e.key === "Escape") setTriSelected(null);
        return;
      }
      if (!total || !album) return;

      if (view === "planches") {
        if ((e.key === "Backspace" || e.key === "Delete") && index >= 0) {
          e.preventDefault();
          apply((a) => removeSpread(a, index));
          setStatus(`Planche ${index + 1} supprimée (⌘Z la ramène)`);
          return;
        }
        // Escape behaves like everywhere else: it lets go of the current
        // selection instead of being swallowed.
        if (e.key === "Escape") {
          setSelected(null);
          setTriSelected(null);
          return;
        }
        // The cover is the light table's page zero: ← reaches it.
        const step = (d: number) =>
          setIndex((i) => Math.min(total - 1, Math.max(COVER, i + d)));
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
          setStatus(`Planche ${to + 1} pleine : aucun gabarit n’accepte une photo de plus`);
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
  }, [
    total,
    apply,
    index,
    selected,
    album,
    view,
    bilan,
    revue,
    curation,
    opened,
    rescue,
    triSelected,
    fire,
    shortcuts,
    signaler,
  ]);

  // The review dies with its subject: leaving the sorting view, or rescuing
  // the last photo, closes it. There is nothing left to review.
  useEffect(() => {
    if (revue === null) return;
    const left = album
      ? triEntries(album, curation, opened?.thumb_srcs ?? []).length
      : 0;
    if (view !== "tri" || left === 0) setRevue(null);
  }, [revue, view, album, curation, opened]);

  if (!album || building) {
    return (
      <>
        <Empty
          onOpen={openAlbum}
          onCreate={createAlbum}
          building={building}
          busyTitle={busyTitle}
          error={error}
          onDismissError={() => setError(null)}
          onCancelBuild={() => void cancelBuild()}
          recents={inTauri ? recents : []}
          onOpenRecent={(dir) => void openRecent(dir)}
        />
        {shortcuts && <RaccourcisView onClose={() => setShortcuts(false)} />}
        {signaler && (
          <SignalerView
            kind={signaler}
            album={null}
            index={-1}
            selected={null}
            onClose={() => setSignaler(null)}
          />
        )}
      </>
    );
  }

  // The album emptied itself, one deletion at a time. Never a mute return
  // to the welcome screen: the way back is spelled out.
  if (total === 0) {
    return (
      <div className="empty">
        <div className="empty-block">
          <p className="kicker">Colophon</p>
          <div className="setup">
            <h1 className="setup-heading">L’album est vide</h1>
            <p className="lede">
              La dernière planche vient d’être supprimée. Rien n’est perdu :
              chaque suppression s’annule.
            </p>
            <p className="setup-actions">
              <button
                className="cta"
                autoFocus
                disabled={(hist?.past.length ?? 0) === 0}
                onClick={undo}
              >
                Ramener la dernière planche (⌘Z)
              </button>
              <button className="link" onClick={() => void closeAlbum()}>
                Composer un autre album
              </button>
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (bilan) {
    return (
      <BilanView
        bilan={bilan}
        album={album}
        curation={curation}
        onOpen={() => setBilan(null)}
        onTri={() => {
          // Straight into the keyboard review: the link promises a review,
          // not a grid to hunt through.
          setBilan(null);
          setView("tri");
          setRevue(0);
        }}
      />
    );
  }

  const onCover = index === COVER && view === "livre";
  const spread = onCover ? null : album.spreads[Math.min(index, total - 1)];
  const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
  const triEntry = entries.find((e) => e.src === triSelected) ?? null;

  return (
    <div className="app">
      <Bar
        album={album}
        dirty={dirty}
        canUndo={(hist?.past.length ?? 0) > 0}
        canRedo={(hist?.future.length ?? 0) > 0}
        view={view}
        triCount={entries.length}
        onView={gotoView}
        onUndo={undo}
        onRedo={redo}
        onSave={() => void save()}
        onRecompose={inTauri ? () => void recompose() : undefined}
        onOpen={inTauri ? undefined : openAlbum}
        onClose={inTauri ? undefined : closeAlbum}
      />
      {view === "tri" ? (
        <TriView
          entries={entries}
          selected={triSelected}
          onSelect={setTriSelected}
          onRescue={rescue}
          onRevue={() => {
            if (!entries.length) return;
            const at = triSelected
              ? entries.findIndex((x) => x.src === triSelected)
              : 0;
            setStatus(null);
            setRevue(Math.max(0, at));
          }}
        />
      ) : view === "envoi" ? (
        <EnvoiView
          album={album}
          printers={printers}
          profil={profil}
          onProfil={setProfil}
          onJump={(planche) => {
            setIndex(planche - 1);
            setSelected(null);
            setView("livre");
          }}
          onExport={() => void regenPdf()}
          exporting={rendering}
          dirty={dirty}
        />
      ) : view === "planches" ? (
        <PlanchesView
          album={album}
          current={index}
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
                printer={printers?.find((p) => p.id === profil) ?? null}
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
                  onCaption={(slot, text) =>
                    apply((a) => setSlotCaption(a, index, slot, text))
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
      {view === "livre" && (
        <ContextLine
          album={album}
          spread={spread}
          onCover={onCover}
          selected={selected}
          onTemplate={(t) => apply((a) => changeTemplate(a, index, t))}
          spreadIndex={index}
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
        />
      )}
      {view === "livre" && !onCover && (
        <Drawer
          entries={entries}
          open={drawerOpen}
          onToggle={() => setDrawerOpen((o) => !o)}
        />
      )}
      {view === "tri" && revue !== null && entries.length > 0 && (
        <RevueView
          entries={entries}
          index={revue}
          status={status}
          onIndex={(i) => {
            if (i >= entries.length) {
              setRevue(null);
              setStatus("Revue terminée, chaque écart est vu");
            } else {
              setStatus(null);
              setRevue(i);
            }
          }}
          onRescue={rescue}
          onClose={() => setRevue(null)}
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
                : "Planche de texte insérée : double-clic pour l’ouvrir et écrire",
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
      ) : view === "envoi" ? (
        <footer className="foot envoi-foot">
          <span className="status">{status}</span>
        </footer>
      ) : (
        <BookFoot
          album={album}
          index={index}
          total={total}
          status={status}
          overflow={overflow}
          rendering={rendering}
          onCancelExport={() => void cancelExport()}
          onSeek={(i) => setIndex(Math.min(total - 1, Math.max(COVER, i)))}
        />
      )}
      {opened && !opened.root_present && (
        <p className="warn">
          Dossier photo introuvable ({album.root}). L’aperçu tourne sur le cache
          de vignettes, l’export pleine résolution ne marchera pas.
        </p>
      )}
      {error && <FaultBlock fault={error} onDismiss={() => setError(null)} />}
      {shortcuts && <RaccourcisView onClose={() => setShortcuts(false)} />}
      {signaler && (
        <SignalerView
          kind={signaler}
          album={album}
          index={onCover ? -1 : Math.min(index, total - 1)}
          selected={selected}
          onClose={() => setSignaler(null)}
        />
      )}
    </div>
  );
}

/**
 * The bar tells three things apart, left to right: which album (the title),
 * where you are (the tabs, centred, nothing else among them), what you can
 * do to the file (a few quiet actions; everything else lives in the menu).
 * The context of the current spread is not the bar's business any more: it
 * sits on its own line next to the planche, in the book view.
 */
function Bar({
  album,
  dirty,
  canUndo,
  canRedo,
  view,
  triCount,
  onView,
  onUndo,
  onRedo,
  onSave,
  onRecompose,
  onOpen,
  onClose,
}: {
  album: Album;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  view: View;
  triCount: number;
  onView: (v: View) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onRecompose?: () => void;
  /** Browser harness only: the shell reaches these through the menu. */
  onOpen?: () => void;
  onClose?: () => void;
}) {
  return (
    <header className="bar">
      <h1>{album.title}</h1>
      <p className="meta">
        <span className="views" role="tablist">
          <button
            className={"view-tab" + (view === "livre" ? " active" : "")}
            onClick={() => onView("livre")}
            aria-keyshortcuts="Meta+1"
            title="⌘1"
          >
            Livre
          </button>
          <button
            className={"view-tab" + (view === "tri" ? " active" : "")}
            onClick={() => onView("tri")}
            aria-keyshortcuts="Meta+2"
            title="⌘2"
          >
            Tri · {triCount}
          </button>
          <button
            className={"view-tab" + (view === "planches" ? " active" : "")}
            onClick={() => onView("planches")}
            aria-keyshortcuts="Meta+3"
            title="⌘3"
          >
            Planches
          </button>
          <button
            className={"view-tab" + (view === "envoi" ? " active" : "")}
            onClick={() => onView("envoi")}
            aria-keyshortcuts="Meta+4"
            title="⌘4 · le contrôle avant impression"
          >
            Envoi
          </button>
        </span>
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
            title="Recompose l’album ; les planches éditées ou verrouillées sont conservées"
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
        {onClose && onOpen && (
          <>
            <span className="actions-sep" aria-hidden="true" />
            <button
              className="link"
              onClick={onClose}
              title="Fermer et composer un autre album"
            >
              Nouveau
            </button>
            <button className="link" onClick={onOpen} title="⌘O">
              Ouvrir
            </button>
          </>
        )}
      </p>
    </header>
  );
}

/**
 * The book view's context line, between the planche and its foot: which
 * template, edited badge, padlock, and the hint of the moment (crop gestures
 * on a selection, the drawer's whereabouts on the cover). Fixed height, so
 * the planche above never moves.
 */
function ContextLine({
  album,
  spread,
  onCover,
  selected,
  spreadIndex,
  onTemplate,
  onLock,
}: {
  album: Album;
  spread: Spread | null;
  onCover: boolean;
  selected: number | null;
  spreadIndex: number;
  onTemplate: (t: string) => void;
  onLock?: () => void;
}) {
  const photoSpread =
    spread && spread.template !== "vide" && spread.template !== "texte";
  return (
    <div className="context-line">
      {onCover ? (
        <span className="context-hint">
          La couverture : titre et sous-titre en place, glissez la photo pour
          la recadrer. Le tiroir de photos revient sur les planches.
        </span>
      ) : (
        spread && (
          <>
            {photoSpread && (
              <TemplatePicker
                album={album}
                spread={spread}
                index={spreadIndex}
                onPick={onTemplate}
              />
            )}
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
            <span className="context-hint">
              {selected !== null
                ? "Recadrage : glisser déplace, molette zoome, ⌥ affine, ⌫ retire la photo"
                : ""}
            </span>
          </>
        )
      )}
    </div>
  );
}

/**
 * The book's foot: page-turn arrows, a ruler graduated one tick per spread
 * (chapter starts marked in accent, their caption on hover), the cover
 * tick, the position, and a fixed line for statuses. The ruler navigates
 * and nothing else: reordering lives in the Planches view, its one home.
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
  onSeek,
}: {
  album: Album;
  index: number;
  total: number;
  status: string | null;
  overflow: string | null;
  rendering: boolean;
  onCancelExport: () => void;
  onSeek: (i: number) => void;
}) {
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
          <Chevron dir="left" />
        </button>
        <button
          className={"ruler-cover" + (index === COVER ? " current" : "")}
          onClick={() => onSeek(COVER)}
          title="Couverture"
          aria-label="Couverture"
        >
          <CoverGlyph />
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
              title={(s.caption ? `${s.caption} · ` : "") + `planche ${i + 1}`}
              onClick={() => onSeek(i)}
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
          <Chevron dir="right" />
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
              Annuler l’export
            </button>
          </span>
        ) : (
          <span className={overflow && !status ? "foot-overflow" : undefined}>
            {status ?? overflow ?? ""}
          </span>
        )}
      </div>
    </footer>
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
          + Planche vide
        </button>
        <button className="link" onClick={() => onInsert("texte")} title="Après la planche courante">
          + Planche de texte
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

/** French names for the engine's format identifiers. The identifier stays
 *  in the data; the screen speaks French. */
const FORMAT_LABELS: Record<string, string> = {
  "carre-21": "Carré 21 × 21",
  "carre-30": "Carré 30 × 30",
  "portrait-a4": "Portrait A4",
  "paysage-a4": "Paysage A4",
  "paysage-28x21": "Paysage 28 × 21",
  "portrait-20x25": "Portrait 20 × 25",
};

function Empty({
  onOpen,
  onCreate,
  building,
  busyTitle,
  error,
  onDismissError,
  onCancelBuild,
  recents,
  onOpenRecent,
}: {
  onOpen: () => void;
  onCreate: (
    dir: string,
    format: string,
    spreads: number,
    densite: string,
    title: string | null,
  ) => void;
  building: string[] | null;
  busyTitle: string | null;
  error: Fault | null;
  onDismissError: () => void;
  onCancelBuild: () => void;
  recents: RecentAlbum[];
  onOpenRecent: (dir: string) => void;
}) {
  const [formats, setFormats] = useState<FormatPreset[]>([]);
  const [densities, setDensities] = useState<DensitePreset[]>([]);
  const [dir, setDir] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [format, setFormat] = useState("carre-21");
  const [spreads, setSpreads] = useState(48);
  const [densite, setDensite] = useState("equilibree");

  useEffect(() => {
    listFormats().then(setFormats, () => {});
    listDensities().then(setDensities, () => {});
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
                Ouvrir un album existant
              </button>{" "}
              (<kbd>⌘</kbd> <kbd>O</kbd>)
            </p>
            {recents.length > 0 && (
              <div className="recents">
                <h2 className="recents-title">Albums récents</h2>
                <ul className="recents-list">
                  {recents.map((r) => (
                    <li key={r.dir}>
                      <button
                        className="recent"
                        onClick={() => onOpenRecent(r.dir)}
                        title={r.dir}
                      >
                        <span className="recent-nom">{r.title}</span>
                        <span className="recent-dir">{r.dir}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}

        {(dir || building) && (
          <div className="setup-layout">
            {dir && !building && (
              <form
                className="setup"
                onSubmit={(e) => {
                  e.preventDefault();
                  onCreate(dir, format, spreads, densite, title.trim() || null);
                }}
              >
                <h1 className="setup-heading">Nouvel album</h1>
                <p className="setup-folder">
                  <code>{dir}</code>
                  <button type="button" className="link" onClick={() => void pick()}>
                    Changer de dossier
                  </button>
                </p>

                <div className="setup-duo">
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
                        soit {spreads * 2} pages
                      </span>
                    </span>
                  </label>
                </div>

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

                <div className="setup-field">
                  <span className="setup-label">rythme</span>
                  <div className="densites">
                    {densities.map((d) => (
                      <button
                        key={d.id}
                        type="button"
                        className={"densite" + (d.id === densite ? " active" : "")}
                        onClick={() => setDensite(d.id)}
                        aria-pressed={d.id === densite}
                      >
                        <span className="densite-nom">{d.nom}</span>
                        <span className="densite-about">{d.about}</span>
                        <DensiteApercu photos={d.photos_par_planche} />
                      </button>
                    ))}
                  </div>
                  <span className="setup-hint">
                    Le rythme se rejoue à chaque recomposition ; chaque planche
                    reste modifiable une par une.
                  </span>
                </div>

                <p className="setup-actions">
                  <button className="cta" type="submit">
                    Composer l’album
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
                    : `Composition de « ${title.trim() || folderName || "l’album"} »`}
                </h1>
                <BuildProgress lines={building} onCancel={onCancelBuild} />
                <p className="setup-hint">
                  {busyTitle
                    ? "Les planches éditées à la main ou verrouillées sont conservées telles quelles."
                    : "L’analyse des photos ne se fait qu’une fois : recomposer ce dossier sera bien plus rapide."}
                </p>
              </div>
            )}

            {chosen && <FormatSpreadPreview f={chosen} />}
          </div>
        )}

        {error && <FaultBlock fault={error} onDismiss={onDismissError} />}
      </div>
    </div>
  );
}

/** Three spreads' worth of cells at this pace, drawn small. Shows what the
 *  sentence says: how crowded a double page gets. */
function DensiteApercu({ photos }: { photos: number }) {
  // The average, rounded to the shapes a template actually lays out.
  const n = photos <= 2 ? 2 : photos <= 3.5 ? 4 : 6;
  const cols = n === 2 ? 1 : 2;
  return (
    <span className="densite-apercu" aria-hidden="true">
      {[0, 1].map((page) => (
        <span
          key={page}
          className="densite-page"
          style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}
        >
          {Array.from({ length: n / 2 }, (_, i) => (
            <span key={i} className="densite-case" />
          ))}
        </span>
      ))}
    </span>
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
  const pageH = 32;
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
      <span className="format-name">
        {FORMAT_LABELS[f.name] ?? f.name.replace(/-/g, " ")}
      </span>
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
            title="Arrête la composition ; rien n’est écrit"
          >
            Annuler
          </button>
          <span className="build-pct">{Math.round(pct)} %</span>
        </span>
      </p>
      <details className="build-details">
        <summary>Détails techniques</summary>
        <pre className="buildlog">
          {log.length ? log.join("\n") : "lecture du dossier…"}
        </pre>
      </details>
    </div>
  );
}
