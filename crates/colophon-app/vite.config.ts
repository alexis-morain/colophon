import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { execFileSync } from "node:child_process";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import type { Plugin, ViteDevServer } from "vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process is a nodejs global
const devAlbum: string | undefined = process.env.COLOPHON_ALBUM;
// Où « Préparer » écrit au harnais : COLOPHON_PREPARER quand il est posé,
// sinon un dossier du répertoire temporaire du système. Jamais à côté de
// l'album, jamais dans `.albums/`.
const devPreparer: string =
  // @ts-expect-error process is a nodejs global
  process.env.COLOPHON_PREPARER ??
  join(tmpdir(), "colophon-preparer", "Album – Cloudprinter");

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
      // « Préparer pour Cloudprinter » au harnais : `colophon --preparer`, le
      // même `export::preparer` que la fenêtre, dans COLOPHON_PREPARER. La
      // réponse a la forme de la commande Tauri : le nom du dossier, jamais
      // son chemin.
      server.middlewares.use("/__dev/commande", commandeDev(dir));
      server.middlewares.use("/__dev/preparer", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          res.end("POST seulement");
          return;
        }
        console.log(`[preparer] ${devPreparer}`);
        const profil =
          new URL(req.url ?? "", "http://x").searchParams.get("profil") ??
          "cloudprinter";
        const repondre = (out: string) => {
          const p = JSON.parse(out);
          res.setHeader("Content-Type", "application/json");
          res.end(JSON.stringify({ dossier: basename(devPreparer), ...p }));
        };
        try {
          repondre(
            execFileSync(
              engineBinary,
              ["--preparer", devPreparer, "--profil", profil, "-o", dir],
              { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
            ),
          );
        } catch (e: any) {
          // Un bloquant sort en échec, le rapport est sur stdout quand même.
          if (e?.stdout) {
            repondre(e.stdout);
            return;
          }
          res.statusCode = 500;
          res.end(String(e?.stderr || e));
        }
      });
      // Le pack d'ornements, tel que le moteur le lit : le harnais tire du
      // même endroit que la fenêtre, sinon l'un des deux montrerait un pack
      // que l'autre n'a pas.
      // Le dump dépasse le mégaoctet de tampon par défaut de Node depuis que
      // le pack porte trente-cinq dessins (1,9 Mo) : au-delà, execFileSync
      // lève ENOBUFS et le sélecteur se dit vide.
      server.middlewares.use("/__dev/ornements", (_req, res) => {
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--dump-ornements"], {
              encoding: "utf8",
              maxBuffer: 32 * 1024 * 1024,
            }),
          );
        } catch (e) {
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
      // La réserve classée pour une planche (comptée à partir de 1, comme
      // --proposition), lue sur l'album.json du disque : le harnais
      // enregistre avant de demander. Les vignettes se rouvrent, donc des
      // secondes sur un gros dossier.
      server.middlewares.use("/__dev/reserve", (req, res) => {
        const planche =
          new URL(req.url ?? "", "http://x").searchParams.get("planche") ?? "1";
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--reserve", planche, "-o", dir], {
              encoding: "utf8",
            }),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // La fiche d'une photo, par son src : le moteur lit le fichier et le
      // relevé s'il y en a un. Un src qui n'est pas un nom est refusé là,
      // pas ici.
      server.middlewares.use("/__dev/fiche", (req, res) => {
        const src =
          new URL(req.url ?? "", "http://x").searchParams.get("src") ?? "";
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--fiche", src, "-o", dir], {
              encoding: "utf8",
            }),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // Ce que l'alerte de qualité lit de chaque photo : taille d'origine,
      // netteté, exposition, et le seuil de flou du dossier.
      server.middlewares.use("/__dev/releve", (_req, res) => {
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--releve-album", "-o", dir], {
              encoding: "utf8",
              maxBuffer: 64 * 1024 * 1024,
            }),
          );
        } catch (e) {
          res.statusCode = 500;
          res.end(String(e));
        }
      });
      // La date fiable de chaque photo posée, par src, lue sur l'album.json
      // du disque comme la réserve : le harnais enregistre avant de dater.
      server.middlewares.use("/__dev/dates", (_req, res) => {
        try {
          res.setHeader("Content-Type", "application/json");
          res.end(
            execFileSync(engineBinary, ["--dates", "-o", dir], {
              encoding: "utf8",
            }),
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
      // La face d'un bloc, sous la grammaire du moteur et rien d'autre :
      // `font::fichier_objet_valide`, réécrite ici parce que le harnais ne
      // doit pas devenir un lecteur de fichiers. Partie ou mal nommée, c'est
      // le même code de refus que la commande.
      server.middlewares.use("/__dev/police-objet", (req, res) => {
        const fichier = new URL(req.url ?? "", "http://x").searchParams.get("fichier") ?? "";
        try {
          if (!/^objet-[A-Za-z0-9-]{1,64}\.(ttf|otf)$/.test(fichier)) throw new Error();
          const octets = read(fichier);
          res.setHeader("Content-Type", "font/ttf");
          res.end(octets);
        } catch {
          res.statusCode = 404;
          res.end("fichier_absent");
        }
      });
      // The faithful preview reads the album's own PDF. Same closed set of
      // two names as the Tauri command: the harness must not become a file
      // reader either.
      //
      // Table jumelle de `nom_apercu` dans `src-tauri/src/lib.rs` : les deux
      // traduisent les deux mêmes mots, et les deux se relisent ensemble. Ce
      // qu'elles rendent est un **aperçu** — la livraison s'appelle
      // `album-print.pdf` et `album-cover.pdf`, et c'est elle seule que le
      // prévol juge.

      server.middlewares.use("/__dev/pdf", (req, res) => {
        try {
          const quoi = new URL(req.url ?? "", "http://x").searchParams.get("quoi");
          const nom =
            quoi === "album"
              ? "album.pdf"
              : quoi === "couverture"
                ? "album-cover.apercu.pdf"
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

/**
 * Le faux de la commande (K-s3), pour le harnais : les commandes Tauri
 * `commande_*` rejouées en mémoire, sur les réponses du faux CloudCore de
 * K-s2 (`core::commande`, tests) et la forme relevée au sandbox le 03/10.
 * Aucune clé ne voyage, aucun réseau n'est touché, rien ne s'écrit sur le
 * disque : `commande.json` n'existe pas au harnais, la vue vit le temps du
 * serveur. L'état d'une commande avance d'un cran à chaque relecture,
 * 1, 5, 10, 15, 33 puis 501, comme le banc sandbox l'a vu passer.
 */
function commandeDev(dir: string) {
  type Commande = {
    reference: string;
    identifiant: string;
    mode: "Sandbox" | "Reel";
    date: string;
    etat: number | null;
    objets_retires: boolean;
  };
  const memoire: {
    cle: { fin: string; mode: "Sandbox" | "Reel" } | null;
    depot: { endpoint: string; region: string; bucket: string } | null;
    commandes: Commande[];
    quotes: string[];
  } = { cle: null, depot: null, commandes: [], quotes: [] };
  const vue = () => ({
    cle: memoire.cle,
    depot: memoire.depot,
    commandes: [...memoire.commandes].reverse(),
  });
  // Le miroir de `commande::produit_pour`, pour le seul harnais.
  const PRODUITS: Record<string, string> = {
    "210x210": "photobook_cw_s210_s_fc",
    "300x300": "photobook_cw_s300_s_fc",
    "210x297": "photobook_cw_a4_p_fc",
    "297x210": "photobook_cw_a4_l_fc",
    "203.2x254": "photobook_cw_us_8x10_p_fc",
  };
  const option = (reference: string, type_: string, note: string, defaut = false) => ({
    reference,
    type_,
    note,
    defaut,
  });
  const SUITE = [1, 5, 10, 15, 33, 501];
  const actions: Record<string, (b: any) => unknown> = {
    vue: () => vue(),
    cle: (b) => {
      memoire.cle = { fin: String(b.cle).trim().slice(-4), mode: b.mode };
      return vue();
    },
    cle_retirer: () => ((memoire.cle = null), vue()),
    depot: (b) => {
      memoire.depot = { endpoint: b.endpoint, region: b.region || "auto", bucket: b.bucket };
      return vue();
    },
    depot_retirer: () => ((memoire.depot = null), vue()),
    verifier: () => ({
      cloudprinter: memoire.cle ? { ok: true, produits: 1673, erreur: null } : null,
      depot: memoire.depot ? { ok: true, produits: null, erreur: null } : null,
    }),
    offre: () => {
      const album = JSON.parse(readFileSync(join(dir, "album.json")).toString());
      const cle = `${album.trim_mm.w}x${album.trim_mm.h}`;
      const produit = PRODUITS[cle] ?? null;
      return {
        format: cle,
        produit,
        pages: 2 * album.spreads.length,
        papier: produit
          ? option("pageblock_150mcs", "type_main_paper", "Pageblock paper 150gsm Machine Coated Silk")
          : null,
        finitions: produit
          ? [
              option("cover_finish_gloss", "type_book_cover_finish", "Cover lamination Gloss finish", true),
              option("cover_finish_matte", "type_book_cover_finish", "Cover lamination Matte finish"),
            ]
          : [],
        couverture: produit ? "cover_130mcg" : null,
        pays: produit
          ? [
              { code: "BE", nom: "Belgium", etat_requis: false },
              { code: "DE", nom: "Germany", etat_requis: false },
              { code: "FR", nom: "France", etat_requis: false },
            ]
          : [],
      };
    },
    devis: (b) => {
      const n = Number(b.quantite) || 1;
      const quotes = [`faux-${Date.now()}-a`, `faux-${Date.now()}-b`];
      memoire.quotes = quotes;
      return {
        prix: (11.8333 * n).toFixed(4),
        tva: (2.3667 * n).toFixed(4),
        devise: "EUR",
        expire_date: new Date(Date.now() + 48 * 3600 * 1000).toISOString(),
        production_jours: 4,
        expeditions: [
          {
            poids_g: "627",
            offres: [
              { quote: quotes[0], niveau: "cp_limited", service: "Limited", transporteur: "Chronopost - France", prix: "5.6320", tva: "1.1264", devise: "EUR" },
              { quote: quotes[1], niveau: "cp_jamais_vu", service: null, transporteur: null, prix: "9.5", tva: "1.9", devise: "EUR" },
            ],
          },
        ],
      };
    },
    passer: (b) => {
      if (!memoire.quotes.includes(b.quote)) throw new Error("cette expédition n'est pas dans le devis");
      const now = new Date();
      memoire.commandes.push({
        reference: `colophon-${now.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15)}`,
        identifiant: `CP-FAUX-${memoire.commandes.length + 1}`,
        mode: memoire.cle?.mode ?? "Sandbox",
        date: now.toISOString().slice(0, 10),
        etat: null,
        objets_retires: false,
      });
      memoire.quotes = [];
      return vue();
    },
    relire: (b) => {
      for (const c of memoire.commandes) {
        if (b.reference ? c.reference !== b.reference : c.objets_retires) continue;
        if (c.etat !== 500) c.etat = SUITE[Math.min(SUITE.indexOf(c.etat ?? 0) + 1, SUITE.length - 1)];
        if ([100, 500, 501].includes(c.etat ?? 0)) c.objets_retires = true;
      }
      return { vue: vue(), erreur: null };
    },
    annuler: (b) => {
      const c = memoire.commandes.find((x) => x.reference === b.reference);
      if (!c) throw new Error("commande inconnue");
      if ((c.etat ?? 0) >= 30) throw new Error("trop_tard");
      c.etat = 500;
      c.objets_retires = true;
      return { vue: vue(), erreur: null };
    },
  };
  return (req: any, res: any) => {
    const action = (req.url ?? "").replace(/^\//, "").split("?")[0];
    let corps = "";
    req.on("data", (c: Buffer) => (corps += c));
    req.on("end", () => {
      try {
        const faire = actions[action];
        if (!faire) throw new Error(`action inconnue : ${action}`);
        const r = faire(corps ? JSON.parse(corps) : {});
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify(r));
      } catch (e: any) {
        const m = String(e?.message ?? e);
        // Le 409 de CloudCore, tel que la commande Tauri le rend.
        res.statusCode = m === "trop_tard" ? 409 : 500;
        res.end(m);
      }
    });
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
