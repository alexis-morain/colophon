// Single door to the backend. Inside the Tauri window it is the IPC; inside a
// plain browser it is the dev album server declared in vite.config.ts, which
// serves the exact same two things from a folder on disk. That fallback is how
// the book view gets checked without rebuilding the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { Album, Discard, OpenedAlbum, Spread } from "./album";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Native folder picker. In the browser, the dev server's folder is implied. */
export async function pickAlbumFolder(): Promise<string | null> {
  if (!inTauri) return "__dev__";
  return pickFolder("Choisir un dossier d’album");
}

/** Native folder picker for a folder of photos to compose from. In the
 *  browser the whole creation flow runs against the dev album, so any
 *  readable path does; it only feeds the title and the folder line. */
export async function pickPhotosFolder(): Promise<string | null> {
  if (!inTauri) return "~/Photos/corse-2013";
  return pickFolder("Choisir un dossier de photos");
}

async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title });
  return typeof picked === "string" ? picked : null;
}

export type FormatPreset = { name: string; w: number; h: number; about: string };

/** Page format presets, from the engine. A static mirror serves the browser
 *  harness so the creation screen can be worked on without the shell. */
export async function listFormats(): Promise<FormatPreset[]> {
  if (!inTauri) return DEV_FORMATS;
  return invoke<FormatPreset[]>("list_formats");
}

const DEV_FORMATS: FormatPreset[] = [
  { name: "carre-21", w: 210, h: 210, about: "carré 21 × 21, le format d’album courant" },
  { name: "carre-30", w: 300, h: 300, about: "carré 30 × 30, grand format de table" },
  { name: "portrait-a4", w: 210, h: 297, about: "A4 portrait" },
  { name: "paysage-a4", w: 297, h: 210, about: "A4 paysage" },
  { name: "paysage-28x21", w: 280, h: 210, about: "paysage 28 × 21" },
  { name: "portrait-20x25", w: 203, h: 254, about: "portrait 20 × 25, le 8 × 10 pouces" },
];

/** A composition pace offered at the first build, as the engine names it. */
export type DensitePreset = {
  id: string;
  nom: string;
  about: string;
  /** Photos per spread on average, for the small preview beside it. */
  photos_par_planche: number;
};

/** The composition paces, from the engine. Mirrored for the browser harness
 *  the way the formats are, so the creation screen works without the shell. */
export async function listDensities(): Promise<DensitePreset[]> {
  if (!inTauri) return DEV_DENSITES;
  return invoke<DensitePreset[]>("list_densities");
}

const DEV_DENSITES: DensitePreset[] = [
  {
    id: "aeree",
    nom: "Aérée",
    about:
      "Une ou deux photos par double page, souvent une seule en grand. Moins de photos retenues, chacune plus grande.",
    photos_par_planche: 2.1,
  },
  {
    id: "equilibree",
    nom: "Équilibrée",
    about:
      "Deux à quatre photos, avec des mosaïques de temps en temps. Le rythme par défaut.",
    photos_par_planche: 3.2,
  },
];

/** The three counts only the engine knows at the end of a build; the
 *  discard detail comes from curation.json. Shown once, before the book. */
export type BuildBilan = {
  photos_scanned: number;
  photos_kept: number;
  chapters: number;
};

/** One proposal composed beside the album, as the creation screen shows it. */
export type VarianteResume = {
  /** Handle: `album.<id>.json` on disk. Empty for the one asked for. */
  id: string;
  nom: string;
  about: string;
  planches: number;
  photos: number;
  /** Three photo sources spread across the book, for the thumbnails. */
  apercu: string[];
};

export type BuiltAlbum = {
  opened: OpenedAlbum;
  bilan: BuildBilan;
  variantes: VarianteResume[];
};

/** Swap in one of the proposals composed beside the album. Reversible until
 *  the first save, which takes the unchosen ones off the disk. */
export async function chooseVariante(id: string): Promise<OpenedAlbum> {
  if (!inTauri) {
    throw new Error(
      "Les propositions vivent dans le dossier de l’album, que le serveur de dev ne sert qu’une fois.",
    );
  }
  return invoke<OpenedAlbum>("choose_variante", { id });
}

/** Build an album from a photo folder, then open it. Long: seconds cold. */
export async function buildAlbum(
  photosDir: string,
  format: string,
  spreads: number,
  densite: string,
  title: string | null,
): Promise<BuiltAlbum> {
  if (!inTauri) return devBuild();
  return invoke<BuiltAlbum>("build_album_from_folder", {
    photosDir,
    format,
    spreads,
    densite,
    title,
  });
}

/** Subscribe to the engine's progress lines. Returns the unsubscribe. */
export async function onBuildProgress(
  cb: (line: string) => void,
): Promise<() => void> {
  if (!inTauri) {
    devProgressListeners.add(cb);
    return () => devProgressListeners.delete(cb);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("build:progress", (e) => cb(e.payload));
}

/* The browser harness stands in for the engine: the same progress lines the
 * Rust side emits, paced so the whole creation flow can be watched and
 * styled without the shell, then the dev album opens as the result. */
const devProgressListeners = new Set<(line: string) => void>();

async function devBuild(): Promise<BuiltAlbum> {
  const emit = (line: string) => devProgressListeners.forEach((cb) => cb(line));
  const tick = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const photos = 575;
  emit(`scan: ${photos} photos`);
  await tick(300);
  for (let i = 20; i <= photos; i += 20) {
    emit(`analyze: ${i}/${photos}`);
    await tick(60);
  }
  emit(`analyze: ${photos} photos, 4.1s`);
  for (const line of [
    "junk: 3 écartées",
    "dedup: 96 doublons",
    "thinning: 95 écartées",
    "chapters: 9 chapitres",
    "layout: 48 planches",
    "curation: 419 entrées",
    "pdf: album.pdf",
  ]) {
    await tick(280);
    emit(line);
  }
  await tick(200);
  const opened = await openAlbum("__dev__");
  // Three proposals from the harness's one album: the counts and the wording
  // are what the screen has to lay out, the photos are the same.
  const avecPhoto = opened.album.spreads.filter((s) => s.slots.length > 0);
  const apercu = [1, 2, 3]
    .map((q) => avecPhoto[Math.floor((avecPhoto.length * q) / 4)])
    .filter(Boolean)
    .map((s) => s.slots[0].src);
  return {
    opened,
    bilan: { photos_scanned: photos, photos_kept: 152, chapters: 9 },
    variantes: [
      {
        id: "autre-rythme",
        nom: "Aérée",
        about:
          "Une ou deux photos par double page, souvent une seule en grand. Moins de photos retenues, chacune plus grande.",
        planches: opened.album.spreads.length,
        photos: 101,
        apercu,
      },
      {
        id: "resserree",
        nom: "Plus court",
        about:
          "Un tiers de planches en moins, donc moins de photos retenues. Un livre qui se feuillette d’un trait, et qui coûte moins cher à imprimer.",
        planches: Math.round(opened.album.spreads.length * 0.68),
        photos: 68,
        apercu,
      },
    ],
  };
}

export async function openAlbum(path: string): Promise<OpenedAlbum> {
  if (inTauri) return invoke<OpenedAlbum>("open_album", { path });
  const res = await fetch("/__dev/album");
  if (!res.ok) throw new Error(await res.text());
  if (!res.headers.get("content-type")?.includes("json")) {
    throw new Error(
      "Serveur de dev sans album : relancez avec COLOPHON_ALBUM=<dossier> npm run dev",
    );
  }
  return res.json();
}

export async function fetchThumb(src: string): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("thumb", { src });
  const res = await fetch(`/__dev/thumb?src=${encodeURIComponent(src)}`);
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** A blocking yes/no question. window.confirm silently returns true inside
 *  the Tauri webview (no JS dialogs in WKWebView), which would turn every
 *  guard into a rubber stamp: the native dialog plugin asks for real. */
export async function confirmDialog(message: string): Promise<boolean> {
  if (!inTauri) return window.confirm(message);
  const { ask } = await import("@tauri-apps/plugin-dialog");
  return ask(message, { title: "Colophon", kind: "warning" });
}

/** Recompose the open album from its photo folder. Edited and locked
 *  spreads survive verbatim; progress streams like a build. Tauri only. */
export async function recomposeAlbum(): Promise<OpenedAlbum> {
  if (!inTauri) {
    throw new Error("recomposition hors application : utilisez la commande colophon");
  }
  return invoke<OpenedAlbum>("recompose_album");
}

/** Abandon the composition in flight. The engine stops between photos. */
export async function cancelBuild(): Promise<void> {
  if (!inTauri) return;
  return invoke("cancel_build");
}

/** Abandon the print render in flight. No half-written PDF can survive. */
export async function cancelExport(): Promise<void> {
  if (!inTauri) return;
  return invoke("cancel_export");
}

/** EXIF date of a photo, formatted for a caption suggestion, or null. */
export async function captionSuggestion(src: string): Promise<string | null> {
  if (!inTauri) return null;
  return invoke<string | null>("caption_suggestion", { src });
}

/** The caption proposed for a spread whose caption is empty: town when it
 *  diverges from the chapter, day when the chapter covers several. Null is
 *  silence, and silence is a full answer. */
export async function legendeProposee(planche: number): Promise<string | null> {
  if (inTauri) return invoke<string | null>("proposition_legende", { planche });
  const res = await fetch(`/__dev/proposition?planche=${planche + 1}`);
  if (!res.ok) return null;
  return res.json();
}

/** Face-anchored focal point, recomputed on the thumbnail. The crop
 *  editor's double-click recentres on it. */
export async function detectedFocal(src: string): Promise<[number, number]> {
  if (!inTauri) return [0.5, 0.42];
  return invoke<[number, number]>("detected_focal", { src });
}

/** The photos curation set aside. Empty for albums built before the export. */
export async function fetchCuration(): Promise<Discard[]> {
  if (inTauri) return invoke<Discard[]>("curation");
  const res = await fetch("/__dev/curation");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** Overwrite album.json, atomically on both sides of the bridge. */
export async function saveAlbum(album: Album): Promise<void> {
  if (inTauri) return invoke("save_album", { album });
  const res = await fetch("/__dev/album", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(album),
  });
  if (!res.ok) throw new Error(await res.text());
}

/** A printer profile, exactly as the engine holds it. Never restated here:
 *  a supplier's specs live in `printer.rs` and travel across the bridge. */
export type Printer = {
  id: string;
  nom: string;
  pdf_x: "x4" | "aucun";
  espace: "rgb" | "fogra39";
  bleed_mm: { haut: number; bas: number; exterieur: number; dos: number };
  safe_mm: number;
  fichiers: "un" | "deux";
  /** The supplier reads one PDF page as one book page, which our
   *  spread-composed interior does not do yet. */
  pages_simples: boolean;
  dos: { mode: "fourni" } | { mode: "calcule"; mm_par_feuille: number; constante_mm: number; certitude: Certitude };
  pages_min: number;
  pages_max: number;
  pas_pagination: number;
  min_ppi: number;
  certitude: Certitude;
  reserves: string[];
};

export type Certitude = "confirme" | "provisoire";

/** One thing wrong with the file, named the way a human would name it. */
export type Defaut = {
  regle: string;
  bloquant: boolean;
  /** 1-based, as the ruler shows it. */
  planche?: number;
  case?: number;
  src?: string;
  cause: string;
  remede: string;
};

/** The sheet handed to whoever receives the PDF. */
export type Fiche = {
  imprimeur: string;
  format_page_mm: [number, number];
  planches: number;
  pages_interieur: number;
  /** Pages in the delivered PDF, cover leaves included when there is one
   *  file: the count to declare at the order. */
  pages_fichier: number;
  fond_perdu_mm: { haut: number; bas: number; exterieur: number; dos: number };
  zone_sure_mm: number;
  espace: "rgb" | "fogra39";
  output_intent: string;
  conformite: "x4" | "aucun";
  fichiers: "un" | "deux";
  dos_mm?: number;
  grammage_g_m2: number;
  resolution_cible_dpi: number;
};

export type PrevolReport = {
  album: string;
  profil: string;
  ok: boolean;
  bloquants: number;
  avertissements: number;
  fiche: Fiche;
  reserves?: string[];
  notes?: string[];
  defauts: Defaut[];
};

/** The printer profiles the engine knows. Outside the shell they come from
 *  the dev album server, which runs the same engine: no second copy of a
 *  supplier's specs anywhere in the front end. */
export async function listPrinters(): Promise<Printer[]> {
  if (inTauri) return invoke<Printer[]>("list_printers");
  const res = await fetch("/__dev/printers");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** One audit counter, counts only. The engine's report also carries the
 *  finding details, which may name photos: the report panel never quotes
 *  them, the numbers alone travel. */
export type AuditCounter = { count: number; seuil: number; dur: boolean };

export type AuditSummary = {
  ok: boolean;
  planches: number;
  compteurs: Record<string, AuditCounter>;
  notes?: string[];
};

/** The raw material of a problem report, gathered on this machine and shown
 *  in full before anything is sent anywhere. */
export type ReportData = {
  version: string;
  os: string;
  /** Last log lines, paths already reduced to file names at write time. */
  log: string;
  /** Null without an album or when the audit fails: the report says so. */
  audit: AuditSummary | null;
};

export async function reportData(): Promise<ReportData> {
  if (inTauri) return invoke<ReportData>("report_data");
  const compteur = (count: number, seuil: number, dur: boolean) => ({
    count,
    seuil,
    dur,
  });
  return {
    version: "dev",
    os: "harnais navigateur",
    log: [
      "2026-08-16 10:02:11 démarrage, version dev",
      "2026-08-16 10:02:40 scan: 575 photos",
      "2026-08-16 10:02:44 layout: 48 planches",
      "2026-08-16 10:03:02 export 300 dpi, profil cloudprinter",
      "2026-08-16 10:05:19 export terminé",
    ].join("\n"),
    audit: {
      ok: true,
      planches: 48,
      compteurs: {
        visage_coupe: compteur(0, 0, true),
        orientation_trahie: compteur(0, 0, true),
        doublon_planche: compteur(0, 0, true),
        sous_resolution: compteur(1, 3, false),
        chapitre_orphelin: compteur(0, 0, false),
        ouverture_faible: compteur(0, 2, false),
        rythme_plat: compteur(0, 1, false),
        legende_manquante: compteur(2, 4, false),
        legende_sur_photo: compteur(0, 0, true),
        repetition_gabarit: compteur(0, 0, true),
      },
    },
  };
}

/** Open the pre-filled issue form. In the shell a guarded Rust command hands
 *  the URL to the system browser; the harness opens a tab. */
export async function openReportUrl(url: string): Promise<void> {
  if (!inTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  return invoke("open_report_url", { url });
}

/**
 * Ask the release feed whether a newer version exists. Returns its version
 * number, or null when there is nothing (and when there is no network, and
 * when the feed cannot be read): an app that cannot reach GitHub is an app
 * that works, and saying so out loud at every launch would be noise.
 *
 * Nothing is downloaded here. The download and the restart are a deliberate
 * click, in the notice this returns.
 */
export async function checkUpdate(): Promise<{
  version: string;
  notes: string;
  install: () => Promise<void>;
} | null> {
  if (!inTauri) return null;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const maj = await check();
    if (!maj) return null;
    return {
      version: maj.version,
      notes: maj.body ?? "",
      install: async () => {
        await maj.downloadAndInstall();
        const { relaunch } = await import("@tauri-apps/plugin-process");
        await relaunch();
      },
    };
  } catch {
    // Offline, feed unreachable, signature refused: all the same to the
    // user, who did not ask. The next launch will try again.
    return null;
  }
}

/** What the About screen shows: the version, and the third-party notices
 *  generated from the two lock files and embedded in the binary. */
export type AboutData = { version: string; notices: string };

export async function aboutData(): Promise<AboutData> {
  if (!inTauri) {
    return {
      version: "dev",
      notices:
        "Les notices sont générées à la compilation (scripts/notices.sh) et " +
        "embarquées dans le binaire : le harnais navigateur n’en a pas.",
    };
  }
  return invoke<AboutData>("about_data");
}

/** Re-render album.pdf, the preview file, from the saved album.json. Seconds
 *  on a fifty-spread album: it draws from the thumbnail cache. */
export async function renderPdf(): Promise<string> {
  if (!inTauri) {
    throw new Error("Le rendu du PDF se fait dans l’application, pas au navigateur.");
  }
  return invoke<string>("render_pdf");
}

/** Raw bytes of one of the album's own PDFs, for the faithful preview. The
 *  two names are a closed set on the Rust side: no path travels here. */
export async function albumPdfBytes(
  quoi: "album" | "couverture",
): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("album_pdf_bytes", { quoi });
  const res = await fetch(`/__dev/pdf?quoi=${quoi}`);
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** Render the flat cover sheet into the album folder, for its preview. Same
 *  renderer the export uses, same profile. */
export async function renderCoverPreview(profil: string): Promise<string> {
  if (!inTauri) {
    throw new Error("La couverture se rend dans l’application, pas au navigateur.");
  }
  return invoke<string>("render_cover_preview", { profil });
}

/** The colophon page, rendered from the facts the album carries. Null on an
 *  album composed before the page existed: nothing can be invented after the
 *  fact, so the Envoi screen simply does not offer it. */
export async function colophonSpread(album: Album): Promise<Spread | null> {
  if (!inTauri) {
    // The harness has no engine; the shape is enough to work the screen.
    if (!album.colophon) return null;
    return {
      template: "colophon",
      slots: [],
      text:
        "Colophon\n\n152 photographies retenues sur 575, prises du 21 au 29 octobre 2013.\n" +
        "Porto-Vecchio et Bonifacio.\nCanon EOS 550D.\n\n" +
        "Composé le 17 août 2026 avec Colophon dev.\n210 × 210 mm, papier 150 g/m².",
    };
  }
  return invoke<Spread | null>("colophon_spread", { album });
}

/** The half-title page, rendered from the facts the album carries and from
 *  the title it carries right now. Null on an album composed before the
 *  facts existed: nothing can be invented after the fact, so the Envoi
 *  screen simply does not offer it. */
export async function gardeSpread(album: Album): Promise<Spread | null> {
  if (!inTauri) {
    // The harness has no engine; the shape is enough to work the screen.
    if (!album.colophon) return null;
    return {
      template: "garde",
      slots: [],
      text: `${album.title}\n\nDu 21 au 29 octobre 2013\nPorto-Vecchio, Bonifacio`,
    };
  }
  return invoke<Spread | null>("garde_spread", { album });
}

/** The composer's own version of one spread, for « rendre à l'automatique ».
 *  Null when the spread was inserted by hand: nothing automatic proposed it.
 *  Throws when the album predates album.origin.json, and the message says so.
 *  Nothing is written: the caller applies it through the undo stack. */
export async function originSpread(
  album: Album,
  index: number,
): Promise<Spread | null> {
  if (!inTauri) {
    throw new Error(
      "La version automatique vit dans album.origin.json, que le serveur de dev ne sert pas.",
    );
  }
  return invoke<Spread | null>("origin_spread", { album, index });
}

/** One album folder as the storage panel shows it. The three weights are
 *  separated because they are not equally expensive to lose. */
export type AlbumEntry = {
  id: string;
  title: string;
  /** Page format in millimetres. Null when album.json could not be read. */
  format: [number, number] | null;
  spreads: number | null;
  /** Seconds since the epoch. */
  modified: number | null;
  bytes_total: number;
  bytes_thumbs: number;
  bytes_pdf: number;
  /** Set when album.json is unreadable: the row still shows and can be
   *  deleted, which is exactly what such an album is good for. */
  probleme: string | null;
};

export type StorageReport = {
  dir: string;
  total: number;
  albums: AlbumEntry[];
};

/** What the app has written on this disk. Walks the data directory, so it
 *  runs off the main thread on the Rust side. */
export async function listAlbums(): Promise<StorageReport> {
  if (!inTauri) return DEV_STORAGE;
  return invoke<StorageReport>("list_albums");
}

const DEV_STORAGE: StorageReport = {
  dir: "~/Library/Application Support/fr.morain.colophon",
  total: 702 * 1024 * 1024,
  albums: [
    {
      id: "random-2024-55846e90",
      title: "random-2024",
      format: [210, 210],
      spreads: 44,
      modified: 1786539600,
      bytes_total: 200 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 46 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "corse-2013-88f933b1",
      title: "Corse 2013",
      format: [210, 210],
      spreads: 48,
      modified: 1786366800,
      bytes_total: 183 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 44 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "mauritanie-2019-9ed43672",
      title: "mauritanie-2019",
      format: [297, 210],
      spreads: 30,
      modified: 1786107600,
      bytes_total: 136 * 1024 * 1024,
      bytes_thumbs: 104 * 1024 * 1024,
      bytes_pdf: 31 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "froid-2013-ea70098b",
      title: "froid-2013",
      format: null,
      spreads: null,
      modified: null,
      bytes_total: 183 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 44 * 1024 * 1024,
      probleme: "album.json illisible : EOF while parsing a value",
    },
  ],
};

/** Delete one album folder and return the bytes freed. The photos it was
 *  composed from are never touched: the Rust side cannot reach them. */
export async function deleteAlbum(id: string): Promise<number> {
  if (!inTauri) {
    const i = DEV_STORAGE.albums.findIndex((a) => a.id === id);
    if (i < 0) return 0;
    const [gone] = DEV_STORAGE.albums.splice(i, 1);
    DEV_STORAGE.total -= gone.bytes_total;
    return gone.bytes_total;
  }
  return invoke<number>("delete_album", { id });
}

/** Empty every thumbnail cache, returning the bytes freed. The caches
 *  rebuild themselves at the next open, they are the only thing that does. */
export async function purgeThumbCaches(): Promise<number> {
  if (!inTauri) {
    let freed = 0;
    for (const a of DEV_STORAGE.albums) {
      freed += a.bytes_thumbs;
      a.bytes_total -= a.bytes_thumbs;
      a.bytes_thumbs = 0;
    }
    DEV_STORAGE.total -= freed;
    return freed;
  }
  return invoke<number>("purge_thumb_caches");
}

/** Show the data directory in the system file manager. */
export async function revealDataDir(): Promise<void> {
  if (!inTauri) return;
  return invoke("reveal_data_dir");
}

/** Preflight the saved album against one profile. Seconds on a big album:
 *  it reopens every original to measure the effective resolution. */
export async function preflight(profil: string): Promise<PrevolReport> {
  if (inTauri) return invoke<PrevolReport>("preflight", { profil });
  const res = await fetch(`/__dev/prevol?profil=${encodeURIComponent(profil)}`);
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(text);
  }
}

/** Ask where to keep the PDF (Téléchargements by default), then render it
 *  at print resolution straight to that path. The dialog comes first: the
 *  render reopens every original at 300 dpi and takes minutes, nobody
 *  should wait through it before being asked a question. Progress arrives
 *  as (done, total) photo counts. Returns the chosen path, or null when
 *  the dialog is dismissed. Tauri only: the dev server has no engine. */
export async function exportPdf(
  title: string,
  profil: string,
  onProgress?: (done: number, total: number) => void,
): Promise<string[] | null> {
  if (!inTauri) {
    throw new Error("PDF hors application : utilisez la commande colophon");
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { downloadDir, join } = await import("@tauri-apps/api/path");
  const name = (title.trim() || "album").replace(/[\\/:]+/g, "-");
  const dest = await save({
    title: "Enregistrer le PDF de l’album",
    defaultPath: await join(await downloadDir(), `${name}.pdf`),
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });
  if (!dest) return null;
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<string>("export:progress", (e) => {
    const m = /^render: (\d+)\/(\d+)/.exec(e.payload);
    if (m && onProgress) onProgress(Number(m[1]), Number(m[2]));
  });
  try {
    return await invoke<string[]>("export_pdf", { dest, profil });
  } finally {
    off();
  }
}
