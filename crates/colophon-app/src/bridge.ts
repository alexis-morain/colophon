// Single door to the backend. Inside the Tauri window it is the IPC; inside a
// plain browser it is the dev album server declared in vite.config.ts, which
// serves the exact same two things from a folder on disk. That fallback is how
// the book view gets checked without rebuilding the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { Album, Discard, OpenedAlbum } from "./album";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Native folder picker. In the browser, the dev server's folder is implied. */
export async function pickAlbumFolder(): Promise<string | null> {
  if (!inTauri) return "__dev__";
  return pickFolder("Choisir un dossier d'album");
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
  { name: "carre-21", w: 210, h: 210, about: "carré 21 × 21, le format d'album courant" },
  { name: "carre-30", w: 300, h: 300, about: "carré 30 × 30, grand format de table" },
  { name: "portrait-a4", w: 210, h: 297, about: "A4 portrait" },
  { name: "paysage-a4", w: 297, h: 210, about: "A4 paysage" },
  { name: "paysage-28x21", w: 280, h: 210, about: "paysage 28 × 21" },
  { name: "portrait-20x25", w: 203, h: 254, about: "portrait 20 × 25, le 8 × 10 pouces" },
];

/** Build an album from a photo folder, then open it. Long: seconds cold. */
export async function buildAlbum(
  photosDir: string,
  format: string,
  spreads: number,
  title: string | null,
): Promise<OpenedAlbum> {
  if (!inTauri) return devBuild();
  return invoke<OpenedAlbum>("build_album_from_folder", {
    photosDir,
    format,
    spreads,
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

async function devBuild(): Promise<OpenedAlbum> {
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
  return openAlbum("__dev__");
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

/** Re-render album.pdf, then ask where to keep a copy, Téléchargements by
 *  default: the app's data dir is no place to look for one's own album.
 *  Returns the chosen path, or null when the dialog is dismissed.
 *  Tauri only: the dev server has no engine. */
export async function exportPdf(title: string): Promise<string | null> {
  if (!inTauri) {
    throw new Error("PDF hors application : utilisez la commande colophon");
  }
  await invoke<string>("render_pdf");
  const { save } = await import("@tauri-apps/plugin-dialog");
  const { downloadDir, join } = await import("@tauri-apps/api/path");
  const name = (title.trim() || "album").replace(/[\\/:]+/g, "-");
  const dest = await save({
    title: "Enregistrer le PDF de l'album",
    defaultPath: await join(await downloadDir(), `${name}.pdf`),
    filters: [{ name: "PDF", extensions: ["pdf"] }],
  });
  if (!dest) return null;
  await invoke("export_pdf", { dest });
  return dest;
}
