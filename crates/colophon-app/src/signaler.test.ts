// The report is the one string the app composes to leave the machine: its
// privacy rule (file names only, no path, no caption) and its URL budget
// are tested here, mechanically, on albums built to violate them.

import { describe, expect, it } from "vitest";
import { Album } from "./album";
import { ReportData } from "./bridge";
import { setLangue } from "./i18n";
import { basename, buildReport, fitReport, issueUrl } from "./signaler";

// The assertions below read the French wording; the node test environment
// has no navigator and would default to English.
setLangue("fr");

function album(): Album {
  return {
    version: 1,
    title: "Été à la mer",
    root: "/Users/famille/Photos/Été à la mer",
    trim_mm: { w: 210, h: 210 },
    bleed_mm: 3,
    spreads: [
      {
        template: "trio",
        slots: [
          {
            src: "2013/plage/IMG_0001.jpg",
            focal: [0.5, 0.42],
            caption: "Grand-mère à la plage",
          },
          { src: "IMG_0002.jpg", focal: [0.5, 0.5], zoom: 1.2 },
          { src: "sous-dossier\\IMG_0003.jpg", focal: [0.3, 0.6] },
        ],
        caption: "Corse, juillet 2013",
        edited: true,
      },
    ],
  };
}

function data(logLines: string[] = []): ReportData {
  return {
    version: "0.1.0",
    os: "macos (aarch64)",
    log: logLines.join("\n"),
    audit: {
      ok: false,
      planches: 1,
      compteurs: {
        visage_coupe: { count: 0, seuil: 0, dur: true },
        sous_resolution: { count: 2, seuil: 0, dur: false },
      },
    },
  };
}

describe("basename", () => {
  it("keeps the name whatever separator wrote the source", () => {
    expect(basename("2013/plage/IMG_0001.jpg")).toBe("IMG_0001.jpg");
    expect(basename("sous-dossier\\IMG_0003.jpg")).toBe("IMG_0003.jpg");
    expect(basename("IMG_0002.jpg")).toBe("IMG_0002.jpg");
  });
});

describe("buildReport", () => {
  it("quotes numbers and file names, never a path or a caption", () => {
    const report = buildReport(data(), album(), 0, 1, "recadrage", false);
    expect(report).toContain("IMG_0001.jpg");
    expect(report).toContain("IMG_0003.jpg");
    expect(report).not.toContain("2013/plage");
    expect(report).not.toContain("sous-dossier");
    expect(report).not.toContain("/Users");
    expect(report).not.toContain("Grand-mère");
    expect(report).not.toContain("Corse, juillet");
  });

  it("tells the spread and the reported cell in numbers", () => {
    const report = buildReport(data(), album(), 0, 1, "recadrage", false);
    expect(report).toContain("Planche 1 sur 1, gabarit trio, 3 photos");
    expect(report).toContain("éditée à la main");
    expect(report).toMatch(/case 2 : \d+ × \d+ mm/);
    expect(report).toContain("zoom 1.20");
    expect(report).toContain("sous_resolution 2/0");
    expect(report).toContain("1 compteur au-dessus du seuil");
  });

  it("stands without an album, audit marked unavailable", () => {
    const report = buildReport(
      { ...data(), audit: null },
      null,
      -1,
      null,
      "bug",
      false,
    );
    expect(report).toContain("Colophon 0.1.0");
    expect(report).toContain("indisponible");
    expect(report).not.toContain("Planche");
  });
});

describe("issueUrl and fitReport", () => {
  it("targets the template of the variant and prefills the spread", () => {
    const url = issueUrl("planche", "rapport", 4, null);
    expect(url).toContain("template=2-bad-spread.yml");
    expect(url).toContain("spread=5");
    expect(url.startsWith("https://github.com/alexis-morain/colophon/issues/new")).toBe(
      true,
    );
  });

  it("shrinks the log until the URL fits, report and URL staying one string", () => {
    const noise = Array.from({ length: 200 }, (_, i) => `ligne ${i} ${"x".repeat(120)}`);
    const { report, url } = fitReport(data(noise), album(), 0, null, "planche", false);
    expect(url.length).toBeLessThanOrEqual(7500);
    expect(url).toContain(encodeURIComponent("Planche 1 sur 1").replace(/%20/g, "+"));
    expect(report).toContain("Extrait du log");
  });
});
