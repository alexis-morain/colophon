# Colophon

**Turn a folder of photos into a print-ready album in under a minute.**
Free, offline, open source. Your photos never leave your machine, the album is
a file you own, and you print it wherever you like.

What Darktable is to Lightroom, Colophon is to Blurb.

> **Status:** 1.0, the first public release. The release chain builds
> installers for macOS and Windows; the Windows build has not yet been tried
> on real hardware. HEIC, RAW and the Apple Photos import work on macOS only.

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

- **Reads** JPEG and PNG everywhere, straight from the folder. No import
  step, no library, no catalogue. On macOS it also reads HEIC and camera RAW
  (CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2 and friends) through the system
  decoder, ImageIO. On Windows and Linux, HEIC and RAW files are counted and
  named on screen, not decoded: never silently dropped.
- **Imports from Apple Photos** (macOS only): pick an album in your photo
  library and Colophon copies its photographs into a visible folder, then
  composes that folder like any other. It never writes to the library. The
  import refuses the network: photographs that stayed in iCloud are counted
  and named, and downloaded only if you ask.
- **Respects your culling**: star ratings and rejects from XMP sidecars, from
  embedded XMP, or from the Windows rating tag enter the score. A photo you
  rejected in Lightroom never beats one you kept, a starred one gets a boost.
  A Google Takeout keeps its dates and places: its `.json` sidecars fill in
  what the EXIF lost.
- **Curates** the take: near-duplicates, bursts and repeated shots of the
  same scene (the sharpest frame, or the one you starred, stays), panoramas
  that do not fit the page, photos too small to print at the size you chose.
  A folder of scanned prints, with no camera data at all, still makes an
  album: the filter that needs camera data switches itself off, says so, and
  the book follows the file dates.
- **Composes** spreads under hard constraints, not vibes. See below.
- **Proposes three albums**, not one: the same photographs at two different
  paces and at two different lengths, composed from a single analysis. You
  pick one; the other two wait on disk until your first edit.
- **Exports** a 300 dpi print-ready PDF with its typeface embedded, plus a
  light preview PDF for the screen.
- **Shows you the file, not a drawing of it.** The editor draws in the DOM,
  the press reads a PDF, and those two can never agree by construction. So
  ⇧⌘P stops drawing and renders the PDF itself, page by page.
- **Opens like a book.** A half-title on page one: the album's title, the
  dates of the trip, the towns crossed. Three lines, nothing else, taken
  from what the composition measured, never a sentence written for you. On
  by default, one click to remove in Send.
- **Signs its own work.** A last, quiet page says what only the machine
  knows: how many photographs were kept out of how many read, over what
  span, in which towns, with which cameras. On by default, one click to
  remove, never a file path or a coordinate on it.
- **Lets you fix anything**: crop by hand inside any cell, swap or replace a
  photo, rescue a discarded one, reorder or duplicate a spread, change a
  spread's layout from at most 23 named dispositions, write captions, rename
  chapters, edit the cover. Adjust exposure and contrast, or turn a photo
  black and white, without a byte of the original changing. Undo covers all
  of it.
- **Places free text and ornaments.** A text block goes anywhere on a
  spread: you move it, resize it by a corner, turn it to any angle, and set
  its size, leading and alignment. Its text wraps inside its box; what does
  not fit runs past the bottom and the editor says so, and nothing is ever
  cut. Typographic ornaments (fleurons, rules, dividers) come from a pack
  shipped with the app, CC0 or public domain, and move, resize and turn the
  same way, keeping their proportions. The pack holds 35 of them in three
  groups: 14 fleurons, 13 rules and 8 dividers. A new block or ornament is
  placed clear of the photographs when the page has room. The editor stops
  an object at the fold and warns when one enters the safe area.
- **Sets the book in the typeface you choose.** In Format & type: a
  shortlist of ten families, one per voice, each name written in its own
  face, and every typeface installed on the computer one click behind it.
  One typeface for the whole book: captions, chapter titles, half-title,
  colophon, cover and spine. It is copied into the album's folder, so the
  album opens and prints the same on a machine that does not have it. A
  typeface whose licence forbids embedding stays in the list, greyed out,
  with the reason. Nothing is recomposed; only the line breaks follow. A
  title prints every character the typeface draws, Zażółć included.

Six page formats out of the box (21×21, 30×30, A4 portrait and landscape,
28×21, 20×25 cm). The command line also takes any size you type in
millimetres; the app sticks to the six. A composed album can switch to
another format without being recomposed: same spreads, same order, same
photos, same crops. Only the layouts whose photos would no longer fit their
cells are replaced, the change is listed before you apply it, and ⌘Z undoes
it.

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

Nor does the editor lose your work behind your back. Quitting, closing the
window and opening another album all ask before discarding unsaved changes.
Saving never deletes a file it did not write itself, so a copy of
`album.json` you made by hand survives. Composing a folder a second time
never overwrites the album already made from it: the new one is written next
to it, and the app offers to reopen the old one first.

## The linter

`colophon --audit` runs fourteen counters over a finished album.

Ten of them judge the composer, and each has a tolerance: cropped face,
betrayed orientation, duplicate spread, under-resolution, orphan chapter,
weak opening, flat rhythm, missing caption, caption over a face, template
repetition. When one goes past its tolerance, the audit exits non-zero. Six
tolerate nothing; orphan chapter, flat rhythm and template repetition allow
one; under-resolution allows three cells before it trips. The print
preflight allows none.

Four count without deciding. Three judge what a hand placed: a free object
inside the safe area, a text block whose text does not fit its box, an
ornament over a photo. The fourth names the characters of the book its
typeface cannot draw, which print as `?`, and which the editor shows as `?`
too. They are listed in the report and never turn it red, because a hand has
the right to set a block to the edge on purpose, and a typeface chosen for a
Japanese album is a choice. What the guillotine cuts through or the binding
splits is stopped at the preflight instead.

That is the quality bar, and it is the same bar in CI. The machine judges the
draft before you have to, and every counter has an obvious manual escape hatch
in the editor. When a class of correction keeps coming back, it becomes a new
counter.

## The Sort view

Every photo the curator dropped is shown, grouped by the reason it was
dropped, with the frame it lost to sitting next to it. A double-click puts it
back.

No other tool tells you *why*. That was the whole point.

## Printing

The export is a plain PDF. Take it to any print shop that accepts one.

The print file declares PDF/X-4 and PDF/A-2b: embedded sRGB output intent,
XMP metadata, embedded fonts, PDF 1.6. PDF/A-2b conformance is measured with
veraPDF; no free validator certifies X-4, so that verdict belongs to your
printer's preflight, and the tool says so instead of pretending.

A preflight check runs against a printer profile before you send anything:
pagination, bleed on each edge, colour space, embedded fonts, effective
resolution cell by cell, the cover, the safe zone. What the guillotine cuts
through or the binding splits blocks the export; what a supplier would
merely rather see further from the edge, like the safe zone, warns. Every
message names the spread and the cause in plain language, never a code, and
tells you the gesture that fixes it. Nothing ever fails silently.

The preflight also opens the files already exported into the album folder.
A PDF whose dimensions belong to another profile blocks. The command line
records every export in `export.json`, next to `album.json`, and Send writes
one into every folder it prepares: the profile it was rendered for and a
fingerprint of the album at that moment. A file rendered for another
printer, or older than the album's last change, blocks too. A file that is
not there says nothing.

```bash
colophon --prevol --profil cloudprinter -o my-album
```

Four profiles ship with the command line: Cloudprinter, Prodigi, Lulu, and a
generic one for the shop down the road. They disagree on bleed, on file count
and on colour space, which is exactly why the profile is data and not a rule
in the code. In the app, Send prepares a folder for Cloudprinter (interior,
cover, `export.json`, and a sheet of what the order form asks for), or writes
one plain PDF for any other shop.

The album is always composed as spreads — nothing crosses the fold, and the
editor shows two facing pages — but the export writes whatever shape the
profile asks for. A shop that imposes spreads gets them whole. A binder that
reads one PDF page as one page of the book, as Cloudprinter and Prodigi do,
gets the interior cut in two on the way out, with the bleed it wants at the
fold, the half-title on page one, and a blank verso closing the block. Same
album, same book, two files.

## Privacy

No account, no login, no sync, no telemetry, ever, not even anonymous, not
even opt-in.

One thing leaves your machine, and you can stop it: at launch, Colophon asks
GitHub whether a newer version exists. That request carries your IP address
and the name of your system, and nothing else; no photograph, no album, no
identifier. Nothing is downloaded or installed without a click, and
Preferences turns the check off. For an application distributed outside any
store, that question is how a fix reaches you, which is why it ships on.

Everything else is offline. Colophon reads your photographs and never modifies
them. The album lives in a single readable `album.json` you can repair with a
text editor. Nothing else leaves your machine unless you deliberately send a
file somewhere, or ask the Photos import to fetch what stayed in iCloud.

## Install

Download the latest release from the
[Releases page](https://github.com/alexis-morain/colophon/releases): a `.dmg`
for macOS, a `.msi` for Windows. Every file ships with its SHA-256 sum, and
the app updates itself from the same place.

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
Yes. The album is a folder on your disk with a readable `album.json` in it,
and the typeface it is set in sits beside it. Nothing is captive, nothing
expires, no project is locked behind a login.

**Is this AI?**
No language model, no prompt, no cloud inference. Local heuristics:
perceptual hashes, a sharpness measure, exposure, face detection. Every
decision is explained in the interface, and you can overrule all of them. An
AI mode may exist one day, with your own API key, and it will never be
required and never decide anything on its own.

**Why not just use Scribus or InDesign?**
Because they are page layout tools and they start from an empty page. The work
Colophon does is choosing which 150 photos out of 600 deserve to be there and
placing them so the book reads. If you want to design each spread by hand,
those tools are better than this one.

**What about my HEIC files?**
On macOS, read natively through the system decoder, ImageIO. No AGPL library
in the way. On Windows and Linux they are not decoded yet: counted and named
on screen, never silently dropped.

**And RAW?**
Same door: the system decoder, which on macOS knows thirty RAW families, CR3
included. Everything up to the print reads the JPEG preview your camera stored
inside the file — its colours, its exposure — and the print itself uses that
preview whenever it holds the resolution floor for its cell; only a cell the
preview cannot fill asks for the sensor. On Windows and Linux, RAW is where
HEIC is: counted and named, never silently dropped.

**How do you make money?**
Not from this. The software is free and stays free, and the full-resolution
PDF export is free and stays free, offline and without an account, whatever
happens next.

**Windows? Linux?**
The release chain builds a Windows installer, and the test suite runs on
Windows, macOS and Linux for every pull request; nobody has yet run the app
on a real Windows machine, so a report from one would be the first. On Windows, HEIC, RAW and the Photos
import are missing, as said above. Linux has no installer: it should build
from source, but nobody has verified the app there yet, and a report either
way would be a welcome contribution.

**Can it do CMYK, layflat, hard covers?**
Not CMYK, not layflat. Colour space and binding options live in the printer
profile, so they arrive one profile at a time, when a real print shop demands
them. The Cloudprinter profile renders the flat cover sheet of its hardcover,
spine and board wrap included.

## Reporting a problem

Three issue templates for a problem: a bug, a bad spread, a bad crop. The app
builds the report for you: the Help menu has one entry per template, the
panel shows you the exact block before anything is sent, and one button opens
the pre-filled issue (or copies the report, if you are offline or without an
account). From the command line, `colophon --audit -o <album>` prints the same
numbers.

A fourth asks for your verdict on the first draft: would you show the album
as the software composed it, and which are its three worst spreads. Send
offers it after an export.

Either way the rule is the same: no photograph, no path, no GPS coordinate,
no caption of yours ever goes in a report; a photo is only ever named by its
file name. Attaching a picture of a spread stays a deliberate act on the
GitHub page, never a default: the app uploads nothing.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Short version: `./scripts/check.sh`
has to stay green, the composer's constants and the linter's thresholds are
one setting split across two files, and the two geometry implementations must
be changed together. Adding an ornament is an entry in
`crates/colophon-core/assets/ornements/pack.toml` and one SVG file, no code,
under CC0 or in the public domain.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

What ships inside the app, each with its licence next to it in
`crates/colophon-core/assets/`:

- the default typeface, Source Sans 3, under the SIL Open Font License;
- the sRGB colour profile the PDF carries, published by the International
  Color Consortium;
- the town names used for chapter titles, from GeoNames, under CC BY 4.0;
- the ornament pack, CC0 or public domain, credited ornament by ornament in
  `assets/ornements/LICENCES.md`.

Third-party code licences are listed in [NOTICES.md](NOTICES.md).
