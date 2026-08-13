// Single door to the backend. Inside the Tauri window it is the IPC; inside a
// plain browser it is the dev album server declared in vite.config.ts, which
// serves the exact same two things from a folder on disk. That fallback is how
// the book view gets checked without rebuilding the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { OpenedAlbum } from "./album";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Native folder picker. In the browser, the dev server's folder is implied. */
export async function pickAlbumFolder(): Promise<string | null> {
  if (!inTauri) return "__dev__";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    directory: true,
    multiple: false,
    title: "Choisir un dossier d'album",
  });
  return typeof picked === "string" ? picked : null;
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
