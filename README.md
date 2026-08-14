# Colophon

Turn a folder of photos into a print-ready album in under a minute. Free, offline, open source.

Colophon is a desktop app for making photo books. Point it at a folder, it curates and lays out a full album on its own, and every decision it makes stays editable. Export a print-ready PDF for any print shop. No account, no subscription, no cloud, and it never touches your original files.

> **Status:** pre-1.0, in active development. macOS today, Windows next. Source will be public at launch.

## What it does

- Reads a folder (JPEG, PNG, HEIC) and builds a complete album layout automatically.
- Curates the take: drops blurry frames, near-duplicates, repeated shots of the same scene, panoramas that do not fit, and photos too low-resolution for the chosen size.
- Composes spreads with a layout engine that respects orientation, faces, rhythm, and chapter structure.
- Exports a 300 dpi print-ready PDF, plus a lightweight preview PDF for the screen.
- Lets you fix anything: swap a photo, remove one, reorder, rescue a discarded shot, all with undo. No dead ends.

## How it works

A single pipeline runs from the folder to the PDF:

```
scan  →  analyze  →  curate  →  compose  →  export
```

Scan reads the files and their metadata. Analyze computes perceptual hashes (dHash plus a DCT pHash), a sharpness score, exposure, and face boxes. Curate removes the frames that would weaken the book. Compose places the survivors on spreads. Export renders either a preview or the full 300 dpi print file.

Everything the pipeline runs on is a 1600 px thumbnail; the original file is only opened at final render, one at a time, to stay light on memory.

## The layout engine

The Composer works under hard constraints, not vibes. Among them:

- Never places a portrait photo in a landscape cell (aspect gap kept at or under 1.4).
- Keeps faces at least 4% clear of any cropped edge, adjusting the crop instead of slicing a head.
- Never puts two near-duplicates, or two shots of the same scene, on the same spread.
- Opens each chapter on a strong photo, drawn from the top quartile.
- Forces a breathing spread (a single photo) at a regular cadence.
- Never repeats the same template four times in a row.
- Keeps every cell at 250 effective ppi or better.

## The linter

`colophon --audit` runs ten counters over a finished album: cropped face, betrayed orientation, duplicate spread, under-resolution, orphan chapter, weak opening, flat rhythm, missing caption, caption over a face, template repetition. It emits JSON, checks each count against a threshold, and exits non-zero if anything fails.

That is the quality bar. The machine judges the draft before you have to, and every counter has an obvious manual escape hatch in the editor.

## Privacy

Fully offline. No account, no telemetry, ever. Colophon reads your photos and never modifies them. The album state lives in a single plain `album.json` you can repair by hand.

## Build from source

Requirements: Rust (stable toolchain), Node 20+, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```bash
# Compose an album from a folder, then lint it
cargo build --release
./target/release/colophon /path/to/photos -o album --format carre-21
./target/release/colophon --audit -o album

# Print-ready 300 dpi render
./target/release/colophon --print -o album

# Desktop editor
cd crates/colophon-app
npm install
npm run tauri dev
```

## Architecture

A Cargo workspace with three crates:

- **`colophon-core`** — the engine: `scan` → `meta` → `thumb` → `analyze` (hashes, sharpness, exposure) → `face` → `heic` (a per-platform system decoder behind a trait) → `pipeline` (curation) → `layout` (the Composer) → `pdf` (geometry and rendering) → `print` (300 dpi) → `audit` (the linter, sharing constants with layout).
- **`colophon-cli`** — the command line, built on clap.
- **`colophon-app`** — a React and Vite interface behind a Tauri shell.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
