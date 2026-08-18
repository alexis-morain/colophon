// The problem report, built entirely on this machine and shown in full
// before the user sends anything. The hard rule of `core::log` applies to
// every line produced here: numbers and file names only. No path, no GPS
// coordinate and no caption ever enters the text, whatever the album holds.

import { Album, mediaCanvas, Rect, slotsFor } from "./album";
import { ReportData } from "./bridge";
import { t } from "./i18n";

/** The three variants, one per issue template shipped in `.github/`. */
export type SignalKind = "bug" | "planche" | "recadrage";

export function signalTitle(kind: SignalKind): string {
  return t(`signaler.titre.${kind}`);
}

const TEMPLATE: Record<SignalKind, string> = {
  bug: "1-bug.yml",
  planche: "2-bad-spread.yml",
  recadrage: "3-bad-crop.yml",
};

const ISSUE_BASE = "https://github.com/alexis-morain/colophon/issues/new";

/** The verdict form of the launch protocol: two questions, no diagnostic.
 *  Offered from the Envoi screen once an export succeeded, because that is
 *  the moment somebody has an album worth judging in front of them. */
export const VERDICT_URL = `${ISSUE_BASE}?template=4-first-draft.yml`;

/** Browsers and GitHub start truncating around 8 KB of URL: the log extract
 *  shrinks until the whole thing fits under this. */
const URL_BUDGET = 7500;

/** File name alone, whichever platform wrote the source string. */
export function basename(src: string): string {
  return src.slice(Math.max(src.lastIndexOf("/"), src.lastIndexOf("\\")) + 1);
}

function shape(r: Rect): string {
  const ratio = r.w / r.h;
  return t(
    ratio > 1.05
      ? "rapport.paysage"
      : ratio < 0.95
        ? "rapport.portrait"
        : "rapport.carree",
  );
}

const mm = (v: number) => Math.round(v);

/** The numbers of one spread: index, template, then each cell's geometry
 *  and the file name of the photo it holds. Captions stay out on purpose. */
function spreadBlock(album: Album, index: number): string[] {
  const spread = album.spreads[index];
  if (!spread) return [];
  const rects = slotsFor(spread.template, spread.slots.length, mediaCanvas(album));
  const lines = [
    t("rapport.planche", {
      n: index + 1,
      total: album.spreads.length,
      gabarit: spread.template,
      photos:
        spread.slots.length > 1
          ? t("rapport.photos", { n: spread.slots.length })
          : t("rapport.photos.une"),
      edition: spread.edited ? t("rapport.editee") : "",
      figee: spread.locked ? t("rapport.figee") : "",
    }),
  ];
  spread.slots.forEach((slot, i) => {
    const r = rects[i];
    const geo = r
      ? `${mm(r.w)} × ${mm(r.h)} mm (${shape(r)})`
      : t("rapport.case.hors.gabarit");
    lines.push(t("rapport.case", { i: i + 1, geo, nom: basename(slot.src) }));
  });
  return lines;
}

/** The numbers of the reported cell: geometry, focal point, manual zoom. */
function cellBlock(album: Album, index: number, cell: number): string[] {
  const spread = album.spreads[index];
  const slot = spread?.slots[cell];
  if (!spread || !slot) return [];
  const r = slotsFor(spread.template, spread.slots.length, mediaCanvas(album))[cell];
  const geo = r
    ? `${mm(r.w)} × ${mm(r.h)} mm (${shape(r)})`
    : t("rapport.hors.gabarit");
  return [
    t("rapport.case.signalee", {
      i: cell + 1,
      geo,
      nom: basename(slot.src),
      fx: slot.focal[0].toFixed(2),
      fy: slot.focal[1].toFixed(2),
      zoom: (slot.zoom ?? 1).toFixed(2),
    }),
  ];
}

function auditBlock(data: ReportData): string[] {
  if (!data.audit) {
    return [t("rapport.audit.indisponible")];
  }
  const entries = Object.entries(data.audit.compteurs);
  const rouges = entries.filter(([, c]) => c.count > c.seuil);
  const verdict = rouges.length
    ? rouges.length > 1
      ? t("rapport.audit.rouges", { n: rouges.length })
      : t("rapport.audit.rouge.un")
    : t("rapport.audit.verts");
  const detail = entries
    .map(([nom, c]) => `${nom} ${c.count}/${c.seuil}`)
    .join(" · ");
  const lines = [
    t("rapport.audit", { n: data.audit.planches, verdict }),
    detail,
  ];
  for (const note of data.audit.notes ?? []) {
    lines.push(t("rapport.audit.note", { note }));
  }
  return lines;
}

/**
 * The whole report, as the panel shows it and as it is sent: same string.
 * `logLines` caps the log extract so the pre-filled URL can shrink to fit.
 */
export function buildReport(
  data: ReportData,
  album: Album | null,
  index: number,
  selected: number | null,
  kind: SignalKind,
  attach: boolean,
  logLines = 30,
): string {
  const blocks: string[][] = [[`Colophon ${data.version}, ${data.os}`]];
  if (album) {
    blocks.push([
      t("rapport.album", {
        w: mm(album.trim_mm.w),
        h: mm(album.trim_mm.h),
        n: album.spreads.length,
      }),
    ]);
  }
  blocks.push(auditBlock(data));
  if (album && index >= 0 && (kind === "planche" || kind === "recadrage")) {
    blocks.push(spreadBlock(album, index));
    if (kind === "recadrage" && selected !== null) {
      blocks.push(cellBlock(album, index, selected));
    }
  }
  if (attach) {
    blocks.push([t("rapport.image")]);
  }
  const log = data.log.split("\n").filter(Boolean);
  const extrait = log.slice(Math.max(0, log.length - logLines));
  if (extrait.length) {
    blocks.push([t("rapport.log"), ...extrait]);
  }
  return blocks
    .filter((b) => b.length)
    .map((b) => b.join("\n"))
    .join("\n\n");
}

/** The pre-filled issue form: template plus its `spread` and `diagnostic`
 *  fields, ids straight from the YAML files. */
export function issueUrl(
  kind: SignalKind,
  report: string,
  index: number,
  selected: number | null,
): string {
  const p = new URLSearchParams({ template: TEMPLATE[kind] });
  if (kind === "planche" && index >= 0) p.set("spread", String(index + 1));
  if (kind === "recadrage" && index >= 0) {
    p.set(
      "spread",
      `Planche ${index + 1}${selected !== null ? `, case ${selected + 1}` : ""}`,
    );
  }
  p.set("diagnostic", report);
  return `${ISSUE_BASE}?${p.toString()}`;
}

/**
 * Report and URL together, the log extract trimmed until the URL fits the
 * budget: what the panel displays is exactly what the button sends.
 */
export function fitReport(
  data: ReportData,
  album: Album | null,
  index: number,
  selected: number | null,
  kind: SignalKind,
  attach: boolean,
): { report: string; url: string } {
  for (let lines = 30; ; lines -= 5) {
    const report = buildReport(data, album, index, selected, kind, attach, lines);
    const url = issueUrl(kind, report, index, selected);
    if (url.length <= URL_BUDGET || lines <= 0) return { report, url };
  }
}
