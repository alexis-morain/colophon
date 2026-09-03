# Colophon

**Turn a folder of photos into a print-ready album in under a minute.**
Free, offline, open source. Your photos never leave your machine, the album is
a file you own, and you print it wherever you like.

What Darktable is to Lightroom, Colophon is to Blurb.

> **Status:** pre-1.0, in active development. macOS today, Windows next.

<!-- CAPTURES, à insérer avant de rendre le dépôt public. Ordre imposé par
     l'audit UX. Ne pas committer ce README avec des images manquantes.

     ![The Sort view: every discarded photo, and why](docs/images/tri.png)
     ![What the composer kept](docs/images/bilan.png)
     ![Composing an album](docs/images/compose.gif)

     1. tri.png     — vue Tri, écartées groupées par raison. Capture héros.
     2. bilan.png   — le bilan de composition, chiffres réels.
     3. compose.gif — dossier choisi jusqu'à la première planche, 10 s max.
     Largeur 1600 px, fenêtre à 1440×900, jeu corse-2013.                  -->

---

## What it does

Point it at a folder. It reads the files, throws out what would weaken the
book, lays out every spread, and hands you a finished draft you can argue
with.

- **Reads** JPEG, PNG, HEIC and camera RAW (CR2, CR3, NEF, ARW, DNG, RAF,
  ORF, RW2 and friends), straight from the folder. No import step, no
  library, no catalogue.
- **Respects your culling**: star ratings and rejects from XMP sidecars, from
  embedded XMP, or from the Windows rating tag enter the score. A photo you
  rejected in Lightroom never beats one you kept, a starred one gets a boost.
- **Curates** the take: blurry frames, near-duplicates, repeated shots of the
  same scene, panoramas that do not fit the page, photos too small to print at
  the size you chose.
- **Composes** spreads under hard constraints, not vibes. See below.
- **Proposes three albums**, not one: the same photographs at two different
  paces and at two different lengths, composed from a single analysis. You
  pick one; the other two wait on disk until your first edit.
- **Exports** a 300 dpi print-ready PDF with the fonts embedded, plus a light
  preview PDF for the screen.
- **Shows you the file, not a drawing of it.** The editor draws in the DOM,
  the press reads a PDF, and those two can never agree by construction. So
  ⇧⌘P stops drawing and renders the PDF itself, page by page.
- **Signs its own work.** A last, quiet page says what only the machine
  knows: how many photographs were kept out of how many read, over what
  span, in which towns, with which cameras. On by default, one click to
  remove, never a file path or a coordinate on it.
- **Lets you fix anything**: crop by hand inside any cell, swap or replace a
  photo, rescue a discarded one, reorder or duplicate a spread, write captions,
  rename chapters, edit the cover. Undo covers all of it.

Six page formats out of the box (21×21, 30×30, A4 portrait and landscape,
28×21, 20×25 cm). The command line also takes any size you type in
millimetres; the app sticks to the six.

## What it guarantees

An auto-layout is judged on its worst spread, so the constraints are code, not
intentions. The composer will never:

- put a portrait photo in a landscape cell, or the reverse;
- slice a detected face at the edge of a cell (4 % clearance, minimum);
- place two near-duplicates, or two shots of the same scene, on one spread;
- repeat the same template four times in a row;
- open a chapter on a weak frame;
- run past the breathing interval of the rhythm you chose without a quiet
  spread.

And it never, under any circumstance, retouches a pixel of your photograph.

## The linter

`colophon --audit` runs ten counters over a finished album and exits non-zero
when one goes past its tolerance: cropped face, betrayed orientation,
duplicate spread, under-resolution, orphan chapter, weak opening, flat rhythm,
missing caption, caption over a face, template repetition. Most counters
tolerate nothing; under-resolution allows three cells before it trips, and the
print preflight allows none.

That is the quality bar, and it is the same bar in CI. The machine judges the
draft before you have to, and every counter has an obvious manual escape hatch
in the editor. When a class of correction keeps coming back, it becomes a new
counter.

## The Sort view

Every photo the curator dropped is shown, grouped by the reason it was
dropped, with the frame it lost to sitting next to it. One click puts it back.

No other tool tells you *why*. That was the whole point.

## Printing

The export is a plain PDF. Take it to any print shop that accepts one.

The print file declares PDF/X-4 and PDF/A-2b: embedded sRGB output intent,
XMP metadata, embedded fonts, PDF 1.6. PDF/A-2b conformance is measured with
veraPDF; no free validator certifies X-4, so that verdict belongs to your
printer's preflight, and the tool says so instead of pretending.

A preflight check runs against a printer profile before you send anything:
pagination, bleed on each edge, colour space, embedded fonts, effective
resolution cell by cell, safe zone. Every message names the spread and the
cause in plain language, never a code, and tells you the gesture that fixes
it. Nothing ever fails silently.

```bash
colophon --prevol --profil cloudprinter -o my-album
```

Four profiles ship today: Cloudprinter, Prodigi, Lulu, and a generic one for
the shop down the road. They disagree on bleed, on file count and on colour
space, which is exactly why the profile is data and not a rule in the code.

The export itself is tuned for one of them, Cloudprinter, and for any shop
that imposes spreads: the interior is composed and rendered as double pages.
Prodigi binds one PDF page to one book page, so the preflight refuses the file
rather than let a book come back bound one page out of place. That is the
preflight doing its job, and it is also the honest state of things: a profile
is a set of checks, not a promise that the file suits every press.

## Privacy

Fully offline. No account, no login, no sync, no telemetry, ever, not even
anonymous, not even opt-in.

Colophon reads your photographs and never modifies them. The album lives in a
single readable `album.json` you can repair with a text editor. Nothing leaves
your machine unless you deliberately send a file somewhere.

## Install

Download the latest release from the
[Releases page](https://github.com/alexis-morain/colophon/releases): a `.dmg`
for macOS today, Windows next. Every file ships with its SHA-256 sum, and the
app updates itself from the same place.

**macOS will refuse to open it the first time.** The app is not signed with an
Apple certificate yet, so Gatekeeper shows "cannot be opened" or "damaged". It
is neither. To open it anyway:

1. Right-click (or Ctrl-click) Colophon.app, choose **Open**.
2. In the dialog, click **Open** again.
3. That's it, and macOS remembers the choice: next time it opens normally.

If the buttons above do not appear (macOS Sequoia and later), go to
**System Settings → Privacy & Security**, scroll down, and click
**Open Anyway** next to the Colophon line.

### Build from source

Requirements: a stable Rust toolchain, Node 20 or newer, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```bash
cargo build --release
```

```bash
./target/release/colophon ~/Pictures/holidays -o album --format carre-21
```

```bash
./target/release/colophon --audit -o album && ./target/release/colophon --print -o album
```

The desktop editor:

```bash
cd crates/colophon-app && npm install && npm run tauri dev
```

## How it works

One pipeline, from the folder to the PDF:

```
scan  →  analyze  →  curate  →  compose  →  export
```

Scan reads files and metadata. Analyze computes two perceptual hashes (dHash
and a DCT pHash), a sharpness score, exposure, and face boxes. Curate removes
the frames that would weaken the book. Compose places the survivors. Export
renders a preview or the full 300 dpi file.

The whole pipeline runs on 1600 px thumbnails. The original file is opened
once, at final render, one at a time, so a 600-photo album does not eat your
memory.

A Cargo workspace with three crates: `colophon-core` is the engine,
`colophon-cli` the command line, `colophon-app` a React interface behind a
Tauri shell.

## Questions people actually ask

**Can I print it wherever I want?**
Yes. The output is a standard PDF at 300 dpi. Colophon has no printing
partner it needs you to use, and puts no watermark, no logo and no barcode on
your book.

**Do my photos go to a cloud?**
No. There is no server. Colophon works with the network switched off, and it
will still work the day this repository stops being maintained.

**Do I keep my file?**
Yes. The album is a folder on your disk with a readable `album.json` in it.
Nothing is captive, nothing expires, no project is locked behind a login.

**Is this AI?**
No model, no prompt, no cloud inference. Local heuristics: perceptual hashes,
a sharpness measure, exposure, face detection. Every decision is explained in
the interface, and you can overrule all of them. An AI mode may exist one day,
with your own API key, and it will never be required and never decide anything
on its own.

**Why not just use Scribus or InDesign?**
Because they are page layout tools and they start from an empty page. The work
Colophon does is choosing which 150 photos out of 600 deserve to be there and
placing them so the book reads. If you want to design each spread by hand,
those tools are better than this one.

**What about my HEIC files?**
Read natively through the system decoder: ImageIO on macOS. The Windows port
will go through WIC, the system decoder there. No AGPL library in the way.

**And RAW?**
Same door: the system decoder, which on macOS knows thirty RAW families, CR3
included. Everything up to the print reads the JPEG preview your camera stored
inside the file — its colours, its exposure — and the print itself uses that
preview whenever it holds the resolution floor for its cell; only a cell the
preview cannot fill asks for the sensor. On Windows, RAW waits for the port
like HEIC does: counted and named, never silently dropped.

**How do you make money?**
Not from this. The software is free and stays free, and the full-resolution
PDF export is free and stays free, offline and without an account, whatever
happens next. An optional ordering feature may come later for people who would
rather click once than deal with a print shop. It will always be optional.

**Windows? Linux?**
Windows is next in line. Linux is untested: it should build from source, but
nobody has verified it yet, and a report either way would be a welcome
contribution.

**Can it do CMYK, layflat, hard covers?**
Not yet. Colour space and binding options live in the printer profile, so they
arrive one profile at a time, when a real print shop demands them.

## Reporting a problem

Three issue templates: a bug, a bad spread, a bad crop. The app builds the
report for you: the Help menu has one entry per template, the panel shows you
the exact block before anything is sent, and one button opens the pre-filled
issue (or copies the report, if you are offline or without an account). From
the command line, `colophon --audit -o <album>` prints the same numbers.

Either way the rule is the same: no photograph, no path, no GPS coordinate,
no caption of yours ever goes in a report; a photo is only ever named by its
file name. Attaching a picture of a spread stays a deliberate act on the
GitHub page, never a default: the app uploads nothing.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Short version: `./scripts/check.sh`
has to stay green, the composer's constants and the linter's thresholds are
one setting split across two files, and the two geometry implementations must
be changed together.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). The embedded typeface is Source
Sans 3 under the SIL Open Font License, its licence sits next to it in
`crates/colophon-core/assets/`.
