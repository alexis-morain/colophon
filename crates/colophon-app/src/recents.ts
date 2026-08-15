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
