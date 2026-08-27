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
_Avoid_: page, sheet, double page

**Cell**:
One rectangle of a spread that holds one photo. A template lays out cells; a
slot fills one.
_Avoid_: frame, box, placeholder

**Slot**:
The photo assigned to a cell, with its framing: a source path, a focal point
and a manual zoom. Stored in `album.json`; the cell it lands in is derived.
_Avoid_: image, picture, photo entry

**Template**:
The named parameters that place a spread's cells: what covers each page, how
slot indices run across the two, and the signed caption height. Sole authority
over every rectangle it generates.
_Avoid_: layout, grid, arrangement

**Trim**:
The finished page, the piece the guillotine leaves. Anything that must survive
the cut is measured from here, never from the media edge.
_Avoid_: page size, final size

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
