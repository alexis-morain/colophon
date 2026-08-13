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
          res.setHeader("Content-Type", "application/json");
          res.end(
            JSON.stringify({ album, dir, root_present: true }),
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
