// The recent-albums list: three entries, newest first, in localStorage.
// Only the Tauri shell writes real paths; the browser harness's « __dev__ »
// album never enters the list.

import { RecentAlbum } from "./menu";

const KEY = "colophon.recents";
const CAP = 3;

export function readRecents(): RecentAlbum[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const list = JSON.parse(raw) as RecentAlbum[];
    return list.filter((r) => r && typeof r.dir === "string" && r.dir !== "");
  } catch {
    return [];
  }
}

/** The folder name of an album path, which is the id the storage panel and
 *  the Rust side both speak. Handles either separator. */
export function albumId(dir: string): string {
  const parts = dir.split(/[\\/]/).filter((p) => p !== "");
  return parts[parts.length - 1] ?? "";
}

/** Drop an album the storage panel just deleted. A recent entry pointing at
 *  a folder that no longer exists would fail loudly at the next click. */
export function forgetRecent(id: string): RecentAlbum[] {
  const list = readRecents().filter((r) => albumId(r.dir) !== id);
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* a full or blocked storage only costs the list */
  }
  return list;
}

export function pushRecent(entry: RecentAlbum): RecentAlbum[] {
  const list = [
    entry,
    ...readRecents().filter((r) => r.dir !== entry.dir),
  ].slice(0, CAP);
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* a full or blocked storage only costs the list */
  }
  return list;
}
