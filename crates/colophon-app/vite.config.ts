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
 * TypeScript port, for every template and every page format.
 */
function geometryParity(): Plugin {
  const binary = join(__dirname, "../../target/release/colophon");
  const FORMATS = ["carre-21", "carre-30", "portrait-a4", "paysage-a4", "240x180"];

  async function check(server: ViteDevServer) {
    const mod = await server.ssrLoadModule("/src/album.ts");
    const problems: string[] = [];

    for (const format of FORMATS) {
      const dump = JSON.parse(
        execFileSync(binary, ["--dump-geometry", "--format", format], {
          encoding: "utf8",
        }),
      );
      const album = { trim_mm: dump.trim_mm, bleed_mm: dump.bleed_mm };
      const g = mod.mediaCanvas(album);
      const near = (a: number, b: number) => Math.abs(a - b) < 1e-6;

      for (const key of ["w", "h", "margin", "gutter"] as const) {
        if (!near(g[key], dump.canvas[key])) {
          problems.push(`${format} canvas.${key}: rust ${dump.canvas[key]}, ts ${g[key]}`);
        }
      }

      for (const [name, want] of Object.entries<any>(dump.templates)) {
        const n = want.slots.length;
        // The port works top-down; flip it back to compare with the PDF.
        const got = mod
          .slotsFor(name, n, g)
          .map((r: any) => [r.x, g.h - (r.y + r.h), r.w, r.h]);
        if (got.length !== n) {
          problems.push(`${format} ${name}: rust ${n} slots, ts ${got.length}`);
          continue;
        }
        want.slots.forEach((slot: number[], i: number) => {
          slot.forEach((v, k) => {
            if (!near(v, got[i][k])) {
              problems.push(
                `${format} ${name} slot ${i}[${"xywh"[k]}]: rust ${v}, ts ${got[i][k]}`,
              );
            }
          });
        });

        const anchor = mod.captionAnchor(name, n, g);
        const tsCaption = [anchor.x, g.h - anchor.y];
        want.caption.forEach((v: number, k: number) => {
          if (!near(v, tsCaption[k])) {
            problems.push(
              `${format} ${name} caption[${"xy"[k]}]: rust ${v}, ts ${tsCaption[k]}`,
            );
          }
        });
      }

      // The fallback rule is written twice too: same parity treatment.
      for (const [name, cap] of mod.TEMPLATES) {
        const want = dump.templates[name];
        if (!want) problems.push(`${format} ${name}: unknown to rust`);
        else if (want.slots.length !== cap) {
          problems.push(
            `${format} ${name} capacity: rust ${want.slots.length}, ts ${cap}`,
          );
        }
      }
      for (const [n, want] of Object.entries<any>(dump.fallbacks ?? {})) {
        const got = mod.templateForCount(Number(n));
        if (!got || got[0] !== want[0] || got[1] !== want[1]) {
          problems.push(
            `fallback(${n}): rust ${JSON.stringify(want)}, ts ${JSON.stringify(got)}`,
          );
        }
      }
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
