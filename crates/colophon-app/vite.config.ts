import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { execFileSync } from "node:child_process";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Plugin, ViteDevServer } from "vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const devAlbum: string | undefined = process.env.COLOPHON_ALBUM;

/**
 * Dev-only album server. With COLOPHON_ALBUM pointing at a folder built by the
 * CLI, `npm run dev` alone is enough to work on the book view in a browser.
 * It mirrors the two Tauri commands and nothing else.
 */
function albumDevServer(dir: string): Plugin {
  const read = (...p: string[]) => readFileSync(join(dir, ...p));
  const engineBinary = join(__dirname, "../../target/release/colophon");
  return {
    name: "colophon-album-dev-server",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__dev/album", (req, res) => {
        // POST mirrors the save_album command: temp file then rename.
        if (req.method === "POST") {
          let body = "";
          req.on("data", (c) => (body += c));
          req.on("end", () => {
            try {
              // Parse first (a broken payload must never touch the file) and
              // re-indent: album.json stays diffable and hand-repairable.
              const pretty = JSON.stringify(JSON.parse(body), null, 2);
              const tmp = join(dir, "album.json.tmp");
              writeFileSync(tmp, pretty);
              renameSync(tmp, join(dir, "album.json"));
              res.end("ok");
            } catch (e) {
              res.statusCode = 500;
              res.end(String(e));
            }
          });
          return;
        }
        try {
          const album = JSON.parse(read("album.json").toString());
          const thumbs = JSON.parse(read("thumbs.json").toString());
          res.setHeader("Content-Type", "application/json");
          res.end(
            JSON.stringify({
              album,
              dir,
              root_present: true,
              thumb_srcs: Object.keys(thumbs),
            }),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      server.middlewares.use("/__dev/curation", (_req, res) => {
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(read("curation.json"));
        } catch {
          // an album built before the export simply has no discard list
          res.setHeader("Content-Type", "application/json");
          res.end("[]");
        }
      });
      // The preflight and the printer list, run by the engine itself. The
      // destination screen is the one view that shows nothing without them,
      // so without this it could only be worked on inside the bundle.
      server.middlewares.use("/__dev/printers", (_req, res) => {
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(execFileSync(engineBinary, ["--profils-json"], { encoding: "utf8" }));
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      server.middlewares.use("/__dev/prevol", (req, res) => {
        const profil =
          new URL(req.url ?? "", "http://x").searchParams.get("profil") ??
          "cloudprinter";
        try {
          // Non-zero exit is the normal answer of a failing preflight: the
          // report is on stdout either way, and it is the report we want.
          const out = execFileSync(
            engineBinary,
            ["--prevol", "--profil", profil, "-o", dir],
            { encoding: "utf8" },
          );
          res.setHeader("Content-Type", "application/json");
          res.end(out);
        } catch (e: any) {
          if (e?.stdout) {
            res.setHeader("Content-Type", "application/json");
            res.end(e.stdout);
            return;
          }
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // The raw geometry dump the editor draws from: the album's own by
      // default, any bare format via ?format=WxH&bleed=N (the creation
      // screen previews formats before an album exists).
      server.middlewares.use("/__dev/geometrie", (req, res) => {
        const url = new URL(req.url ?? "", "http://x");
        const format = url.searchParams.get("format");
        const bleed = url.searchParams.get("bleed");
        try {
          let args: string[];
          if (format) {
            args = ["--dump-geometry", "--format", format];
            if (bleed !== null) args.push("--bleed", bleed);
          } else {
            const album = JSON.parse(read("album.json").toString());
            args = [
              "--dump-geometry",
              "--format",
              `${album.trim_mm.w}x${album.trim_mm.h}`,
              "--bleed",
              String(album.bleed_mm),
            ];
          }
          res.setHeader("Content-Type", "application/json");
          res.end(execFileSync(engineBinary, args, { encoding: "utf8" }));
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // The proposed spread caption, computed by the engine on the album's
      // own EXIF: the ghost text is visible in a browser, not only in the
      // bundle.
      server.middlewares.use("/__dev/proposition", (req, res) => {
        const planche =
          new URL(req.url ?? "", "http://x").searchParams.get("planche") ?? "0";
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(
              engineBinary,
              ["--proposition", planche, "-o", dir],
              { encoding: "utf8" },
            ),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // The templates a spread can switch to, count and orientation both
      // fitting: the engine's one rule. The srcs travel as a JSON array so
      // an unsaved edit filters right.
      server.middlewares.use("/__dev/gabarits", (req, res) => {
        const srcs =
          new URL(req.url ?? "", "http://x").searchParams.get("srcs") ?? "[]";
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--gabarits", srcs, "-o", dir], {
              encoding: "utf8",
            }),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      server.middlewares.use("/__dev/thumb", (req, res) => {
        try {
          const src = new URL(req.url ?? "", "http://x").searchParams.get("src");
          const index = JSON.parse(read("thumbs.json").toString());
          const name = src ? index[src] : undefined;
          if (!name) {
            res.statusCode = 404;
            res.end(`${src} absent de thumbs.json`);
            return;
          }
          res.setHeader("Content-Type", "image/jpeg");
          res.end(read(".cache", "thumbs", name));
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // The bytes the emitter would embed, so the harness measures what the
      // application measures. Same closed set of two names as the engine —
      // `album.json` is hand-editable, and a harness that joined whatever it
      // says to the album folder would be a file reader.
      server.middlewares.use("/__dev/police", (req, res) => {
        try {
          const fichier = new URL(req.url ?? "", "http://x").searchParams.get("fichier");
          res.setHeader("Content-Type", "font/ttf");
          if (fichier === "police.ttf" || fichier === "police.otf") {
            res.end(read(fichier));
            return;
          }
          // No face chosen, or a name we never write: the engine's own.
          res.end(readFileSync(join(__dirname, "public/fonts/SourceSans3-Regular.ttf")));
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // The faithful preview reads the album's own PDF. Same closed set of
      // two names as the Tauri command: the harness must not become a file
      // reader either.

      server.middlewares.use("/__dev/pdf", (req, res) => {
        try {
          const quoi = new URL(req.url ?? "", "http://x").searchParams.get("quoi");
          const nom =
            quoi === "album"
              ? "album.pdf"
              : quoi === "couverture"
                ? "album-cover.pdf"
                : null;
          if (!nom) {
            res.statusCode = 404;
            res.end(`aperçu inconnu : ${quoi}`);
            return;
          }
          res.setHeader("Content-Type", "application/pdf");
          res.end(read(nom));
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
    },
  };
}

/**
 * `pdf.rs::slots_for` and `album.ts::slotsBottomUp` are the same arithmetic
 * written twice, which is exactly how a preview starts lying about the print.
 * GET /__dev/geometry runs the engine's own dump and diffs it against the
 * TypeScript port. The comparison itself lives in src/parity.ts, shared with
 * the Vitest test that runs without a dev server; loaded through
 * ssrLoadModule so an edit to album.ts is picked up without a restart.
 */
function geometryParity(): Plugin {
  const binary = join(__dirname, "../../target/release/colophon");

  async function check(server: ViteDevServer) {
    const parity = await server.ssrLoadModule("/src/parity.ts");
    const problems: string[] = [];
    for (const format of parity.PARITY_FORMATS as string[]) {
      const dump = JSON.parse(
        execFileSync(binary, ["--dump-geometry", "--format", format], {
          encoding: "utf8",
        }),
      );
      problems.push(...parity.geometryProblems(dump, format));
    }
    return problems;
  }

  return {
    name: "colophon-geometry-parity",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__dev/geometry", (_req, res) => {
        check(server).then(
          (problems) => {
            res.setHeader("Content-Type", "application/json");
            res.statusCode = problems.length ? 500 : 200;
            res.end(JSON.stringify({ ok: problems.length === 0, problems }, null, 2));
          },
          (e) => {
            res.statusCode = 500;
            res.end(String(e));
          },
        );
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(),
    geometryParity(),
    ...(devAlbum ? [albumDevServer(devAlbum)] : []),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
