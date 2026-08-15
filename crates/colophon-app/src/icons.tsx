// One icon system: small SVGs drawn in the padlock's line (PlanchesView's
// LockGlyph set the tone), replacing the unicode glyphs ▾ ▴ ‹ › and the
// lettered cover tick. Stroke follows currentColor, so every icon inherits
// the state colours of its button.

export function Chevron({ dir }: { dir: "up" | "down" | "left" | "right" }) {
  const d = {
    up: "M 2.5 7.5 L 6 4 L 9.5 7.5",
    down: "M 2.5 4.5 L 6 8 L 9.5 4.5",
    left: "M 7.5 2.5 L 4 6 L 7.5 9.5",
    right: "M 4.5 2.5 L 8 6 L 4.5 9.5",
  }[dir];
  return (
    <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  );
}

/** The cover as a glyph: a closed board, spine to the left. */
export function CoverGlyph() {
  return (
    <svg viewBox="0 0 12 14" width="11" height="13" aria-hidden="true">
      <rect
        x="1.5"
        y="1.5"
        width="9"
        height="11"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
      />
      <line
        x1="4"
        y1="1.5"
        x2="4"
        y2="12.5"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  );
}
