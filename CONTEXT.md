# Colophon

The words this codebase uses for the things it manipulates. One meaning, one
term, listed once. Code, comments and issue templates use these; a synonym in
a diff is a bug report waiting to happen.

This is a glossary, not a specification. What the project is building lives in
the README; what it decided and why lives in `docs/adr/`.

## The book

**Spread**:
Two facing pages, the unit Colophon composes and prints. Never a single page:
no image crosses the fold, and the PDF's interior is a run of spreads.
_Avoid_: page, double page

**Sheet**:
The piece of paper that turns. Its front is the right page of spread *n*, its
back the left page of spread *n + 1*: one sheet, two spreads, and no image on
either face is cut by the movement. A sheet is not a spread and never stands
in for the word; the cover is a sheet of its own, flat, with the spine down
the middle.
_Avoid_: leaf, folio, page

**Turn**:
One sheet going from flat on one side of the fold to flat on the other.
Started by its corner or by the keyboard, reversible for as long as the finger
is down, and replaced by an instant change for a reader who asked for less
movement.
_Avoid_: flip, page transition, animation

**Cell**:
One rectangle of a spread that holds one photo. A template lays out cells; a
slot fills one.
_Avoid_: frame, box, placeholder

**Slot**:
The photo assigned to a cell, with its framing: a focal, a manual zoom and a
source path. Stored in `album.json`; the cell it lands in is derived.
_Avoid_: image, picture, photo entry

**Focal**:
The point of a photograph a slot is framed on, as a fraction of its width and
its height. A property of the photograph and not of the cell, which is what
lets a crop survive a change of format: the window centres on the focal, and
only the image borders may move it off centre.

Before `album.json` schema 2 the same field held a fraction of the leftover
room inside the cell. That is cell-dependent, so the same number showed a
different part of the photo as soon as the format changed — a bascule
destroyed hand-set framing in silence. An album is converted once, on open,
and stamped; the conversion needs only the two aspect ratios and the zoom.
_Avoid_: offset, crop position, centre of interest

**Template**:
The named parameters that place a spread's cells: what covers each page, how
slot indices run across the two, and the signed caption height. Sole authority
over every rectangle it generates.
_Avoid_: layout, grid, arrangement

**Trim**:
The finished page, the piece the guillotine leaves. Anything that must survive
the cut is measured from here, never from the media edge.
_Avoid_: page size, final size

**Réglage**:
The non-destructive adjustment of one photograph — exposure, contrast, black
and white — stored in `album.json` (`Album::reglages`, keyed by source),
applied where pixels are resolved (screen and export), never an octet on the
original. A property of the photograph, not of the cell: never in `Slot`,
never posing `edited`, invisible to `--reprise` by construction. The
transform is defined once, in `core::reglage`, and it is the CSS filter
formula; `reglage.ts` is its port, held by the LUT parity fixture.
_Avoid_: retouche (already drifting in comments as « any edit »), filtre,
correction

**Bascule**:
Carrying a composed album into another trim. Not a recomposition: every
spread, its order, its photographs and their framing come across untouched,
and only the template of a spread whose photos would betray their new cells is
replaced. It reads no pixel — the photo sizes come from the relevé, or from
the originals' headers — and writes nothing until the editor's own save, so
⌘Z undoes a change of format like any other edit.
_Avoid_: resize, convert, reflow, re-layout

**Aptitude**:
Whether a template can carry given photographs on a given geometry: it holds
all of them, and none betrays its cell past `audit::ASPECT_BETRAYAL`. One
rule, in `gabarit::apte`; the picker, the keyboard cycle and the bascule read
it and none of them rewrites it.
_Avoid_: fit, compatibility, suitability

**Repli**:
The template a spread falls back to when its own stops being apt, always of
the same capacity so no photograph is dropped. When nothing of that capacity
is apt the spread keeps what it has and the bilan names it: a betrayed cell is
visible and one click from being changed, a lost photograph is neither.
_Avoid_: fallback, downgrade, substitution

**Disposition**:
What one entry of the template picker is: how many cells each page of a spread
carries and in how many rows, the caption band and the cell shapes folded
away. Several templates share one — 171 compatible templates on a four-photo
spread are at most 23 dispositions — and the one actually applied is whichever
of them these photographs fit best, by `gabarit::trahison`. The engine knows
nothing of it: it is how the interface makes a catalogue of 209 families
readable. It has no side, the picker flipping it onto the right page from the
spread's parity.
_Avoid_: layout, arrangement, shape, grid

**Spécimen**:
A face's own name, and a line of text, drawn in that face, so a typeface is
seen before it is chosen. The bytes are the ones the emitter would embed —
`police_apercu`, the same extraction as choosing it, writing nothing beside
the album — registered under a family internal to the application: the screen
never names an installed font.
_Avoid_: preview, sample, specimen sheet

**Face**:
One named design inside a font file. `Helvetica.ttc` is a file and carries
several faces; each has its own name, its own metrics, and its own right to
enter a PDF, which the licence bits grant or refuse. The engine addresses a
face by path and index, and reads faces rather than files. A face can be
pulled out of its file as a font file of its own — one face, the tables a PDF
asks for, no glyph touched — which is what lets one that lives in a collection
be embedded at all. The album's own travels inside every PDF it exports.
Reading a face and setting type in it are two different needs: the reader lets
the file go, and a face **opened for writing** keeps it, because every line
asks the character map for glyphs and the file itself goes into the PDF. That
one path both measures and draws, so what an editor shows and what a printer
receives cannot drift apart.
_Avoid_: font, typeface, family, style

**The album's face**:
The one face a book is set in, whatever else the machine carries: captions,
chapter titles, half-title, colophon, cover and spine. It is a property of the
album, never of an object on a page, and it is recorded in `album.json` by the
name of the file **copied beside it** — so a folder carried to another machine
opens and prints the same, and nothing ever looks a face up by name on the
machine that opens it. Absent means the face the engine ships. Changing it
recomposes nothing: same spreads, same photographs, same crops, and only the
line breaks follow the new set widths.
_Avoid_: the album font, the chosen font, embedded font


**Bleed**:
The margin of print beyond the trim, cut away in binding. Photos bleed on
purpose; text never does.
_Avoid_: overprint, margin

**Media box**:
The whole printed surface of a spread: two trimmed pages plus bleed all round.
The rectangle the PDF declares, and the frame every geometry is expressed in.
_Avoid_: canvas, sheet, page box

**Spread geometry**:
The media box of one spread plus the margins and gutter the engine derived
from it. `pdf::geometry` computes it; the editor reads the same numbers from
the geometry dump and declares none of its own.
_Avoid_: canvas, dimensions

## The scene

**Scene**:
Everything visible on one spread, as objects, derived from a spread and its
geometry by a single function. Not stored: a scene is what a template plus
its slots mean, computed the same way for every renderer.
_Avoid_: scene graph, display list, render tree

**Object**:
One visible element of a scene — a photo in its cell, a caption, a text block,
later a free object. It carries a rectangle, a depth, a reading rank and a
role, and nothing else: no rotation, no matrix.
_Avoid_: item, element, node

**Depth**:
Where an object sits in the paint order, back to front. It is the object's
index in the scene, not a field: object *n* prints over object *n − 1*.
_Avoid_: z, layer, stacking order

**Reading rank**:
Where an object comes in the order a person reads the spread, which is not the
depth. Derived from the template's own slot order, which already declares it.
_Avoid_: tab index, focus order

**Anchor**:
Where a set line sits: its baseline. A text object is placed by its anchor and
covers its ink; neither derives from the other.
_Avoid_: position, origin

**Ink**:
The rectangle a set text actually covers, measured in the embedded face.
Distinct from the placement proxy the geometry dump reserves, which has no
album to measure and so must assume a caption of unknown length.
_Avoid_: bounds, text box

**Role**:
What an object is, as a code the engine emits and the interface translates:
`photo`, `photo_caption`, `chapter_caption`, `text`, never a rendered
sentence. Strings born in the engine stay in one language; codes do not.
_Avoid_: kind, label, type name

**Derived object**:
An object a template generates, and which therefore never appears in
`album.json`. Writing one down would create a second source of truth for a
rectangle the template already owns.

**Free object**:
An object no template can generate, placed by hand, which is why it is stored.
None exist yet: they arrive with free text and cliparts.
_Avoid_: custom element, overlay

## The measures

**Composer**:
The pass that turns curated photos into spreads: it chooses templates, assigns
photos to cells, and paces the book. It proposes; every decision it makes can
be overridden by hand.
_Avoid_: layout engine, generator

**Linter**:
The audit that counts a composed album's defects against written thresholds,
and fails when one is crossed. It judges the machine's work, not the file's
validity.
_Avoid_: validator, checker

**Preflight**:
The gate between a composed album and a print order, read against one printer
profile. A blocking defect stops the export.
_Avoid_: prepress check, validation

**Reprise**:
The share of the composer's proposal a human had to correct by hand, measured
as a content diff against `album.origin.json`. The number the GO/NO-GO
milestone hangs on.
_Avoid_: rework rate, edit distance

**Sidecar**:
A file another program wrote next to a photograph and named after it, read
where the photo's own metadata is silent: the `.xmp` of a cataloguing app
(rating), the `.json` of a Google Takeout (capture date, GPS). Found by a
closed list of spellings, never by listing the folder. A sidecar is always
someone else's; what Colophon writes about a photo is a fiche.
_Avoid_: companion file, metadata file

**Preview**:
The JPEG a camera renders and stores inside its RAW file, next to the sensor
data: the camera's own colours and exposure, at the sensor's size on many
bodies and smaller on some. Everything before the print reads it, and the
print does too whenever it holds the resolution floor for its cell; the
sensor is decoded — the platform's rendering, not the camera's — only when
the preview falls short. Resolution is always judged on the sensor.
_Avoid_: thumbnail (that is the album's cache), embedded JPEG, proxy

**Fiche**:
What one reading of one photo measured — metadata, analysis, original size,
faces — exactly as the composer consumes it. A fiche is the `Photo` struct
itself, serialized; never a second model beside it.
_Avoid_: record, entry, sidecar

**Relevé**:
Every fiche one reading of a folder produced, plus what that reading skipped
or could not decode. Serialized, it replays a composition on a machine that
holds no photograph; an album composed that way carries it as `releve.json`,
and the linter measures from it. The fiches of the reference sets live in
`crates/colophon-core/fiches/`.
_Avoid_: dump, snapshot, manifest
