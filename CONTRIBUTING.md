# Contributing to Colophon

Bug reports are worth as much as patches here. The three issue templates tell
you exactly what to paste, and the app's Help menu builds the whole report
for you, one entry per template; outside the app, `colophon --audit` prints
the numbers it quotes. Read on if you want to change the code.

## Build and run

Requirements: a stable Rust toolchain, Node 20 or newer, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your
platform.

```bash
cargo build --release
```

```bash
./target/release/colophon ~/Pictures/some-folder -o .albums/test --format carre-21
```

The editor runs from `crates/colophon-app` with `npm install` then
`npm run tauri dev`.

## The one gate

```bash
./scripts/check.sh
```

It builds, runs the Rust tests, composes the three test sets in six formats
and lints them, measures rework, rasterises the PDF back to PNG and checks
420 cells, then runs `tsc` and vitest. It has to be green before and after
your change. If it is red on a fresh clone, that is a bug, please report it.

Two things it needs to be true: `cargo build --release` must have run first,
because the geometry parity test compares against the real binary, and no
other process may be writing into `.albums/`.

## Four traps worth knowing before you touch anything

**The two geometries, and there are two pairs.** Slot geometry exists twice, in
`pdf.rs::slots_for` and in `album.ts::slotsBottomUp`, including the fallback
table, the template list, the anchors and the caption box. The cover sheet
exists twice as well, spine included: `cover.rs::geometry` and
`album.ts::coverSheet`. The Rust side draws the PDF, the TypeScript side draws
the editor, and the PDF is the one that is right. Change one side and you
change the other in the same commit; the parity test will catch you, but only
after you have wasted an hour.

**The linter and the composer share their constants.** The 250 ppi floor, the
1.4 aspect gap, the duplicate thresholds (24 bits of dHash, 8 of pHash, 180
seconds plus colour distance) live in `audit.rs` and are imported by
`layout.rs`. Moving a threshold on one side without the other makes the check
fail, which is the intended behaviour: they are one setting.

**`album.origin.json` is never rewritten.** It is the composer's original
proposal, and `--reprise` measures how much of it a human had to correct.
Recomposing over it would quietly fold manual fixes into the reference and the
measurement would fall to zero on its own.

**The bundled typeface is an asset of the repository, and it ships twice.**
Source Sans 3 lives in `crates/colophon-core/assets/` with its OFL licence
next to it, embedded in every exported PDF, and again in
`crates/colophon-app/public/fonts/` for the editor. Substitute one and you
substitute both, and any substitute has to pass the tests in `font.rs`: an
`fsType` that allows embedding, and the full French character set. A font
missing the `œ` glyph compiles perfectly and ruins a book.

## What a good patch looks like

Small, and green. One concern per pull request.

Match the code around you rather than the style you prefer: comment density,
naming, and the habit of explaining *why* rather than *what*. Comments in the
codebase are in English.

If your change alters what the composer produces, say so explicitly and post
the audit numbers before and after, on the three test sets. A layout change
that improves one album and quietly degrades another is the failure mode this
project is built to catch.

New behaviour comes with the measurement that proves it. That usually means a
test, sometimes a new counter in the linter.

## What will get declined

Cliparts, stickers, masks, decorative backgrounds, fancy borders. Anything
that retouches the user's pixels. Telemetry, in any form, however anonymous,
however opt-in. Accounts, logins, sync. Onboarding carousels, guided tours,
badges, streaks, rating prompts, promotional banners. Any feature that makes a
project captive.

Some of these are good ideas in other products. They are not this one, and
saying no to them is the design.

## Reporting a problem instead

Three issue templates: a bug, a bad spread, a bad crop. Each one asks for the
album's diagnostic block, which `colophon --audit` prints today and which the
app will one day paste for you, because a report with the album's counters in
it is a report someone can act on, and a free-form form produces "it does not
work".

Suggestions and questions go to Discussions.

## Licence

GPL-3.0-or-later. By contributing you agree your contribution is licensed
under it. Keep the header on files you create.
