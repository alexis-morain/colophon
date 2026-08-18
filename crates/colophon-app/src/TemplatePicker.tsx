// The template picker, drawn instead of named: each choice is a miniature
// spread in the album's own geometry (slotsFor, the engine's arithmetic), so
// picking a layout means seeing it. The engine's template names stay in the
// data; the interface speaks French.
//
// Recto/verso variants are one choice here: the picker shows the family and
// flips it onto the right page from the spread's parity, the way the
// Composer does (`layout.rs::with_flip`). The engine's fallback table is
// untouched; this is a display filter only.

import { useEffect, useRef, useState } from "react";
import { Album, mediaCanvas, slotsFor, Spread, templateCapacity, TEMPLATES } from "./album";
import { templateChoices } from "./edits";
import { Cle, FR, t } from "./i18n";

/** The family behind a template name, verso suffix folded away. */
function familyOf(template: string): string {
  return template.endsWith("_verso")
    ? template.slice(0, -"_verso".length)
    : template;
}

/** The label of a template family lives in the dictionaries (`gabarit.*`);
 *  an engine name without an entry shows raw, which is the honest default. */
export function templateLabel(template: string): string {
  const family = familyOf(template);
  const cle = `gabarit.${family}`;
  return cle in FR ? t(cle as Cle) : family;
}

/** The face a family takes on this spread: verso on odd spreads when the
 *  variant exists, like the Composer's own flip. */
function faceFor(family: string, index: number): string {
  const verso = `${family}_verso`;
  if (index % 2 === 1 && TEMPLATES.some(([t]) => t === verso)) return verso;
  return family;
}

export function TemplatePicker({
  album,
  spread,
  index,
  onPick,
}: {
  album: Album;
  spread: Spread;
  /** Position of the spread in the book, for the recto/verso parity. */
  index: number;
  onPick: (template: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  // Escape and outside clicks close the panel before anything else reacts.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onDown);
    };
  }, [open]);

  // The Planche menu's « Gabarit… » opens the same panel as a click.
  useEffect(() => {
    const onOpen = () => setOpen(true);
    window.addEventListener("colophon:gabarit", onOpen);
    return () => window.removeEventListener("colophon:gabarit", onOpen);
  }, []);

  // One entry per family: the verso variants merge into their family and
  // the parity picks the face at the moment of the choice.
  const families: [string, number][] = [];
  for (const [t, cap] of templateChoices(spread)) {
    const family = familyOf(t);
    if (!families.some(([f]) => f === family)) families.push([family, cap]);
  }

  return (
    <div className="tpl" ref={root}>
      <button
        className="tpl-current"
        onClick={() => setOpen((o) => !o)}
        title={t("gabarit.titre")}
        aria-expanded={open}
      >
        <TemplateDiagram album={album} template={spread.template} width={34} />
        <span>{templateLabel(spread.template)}</span>
      </button>
      {open && (
        <div className="tpl-panel" role="listbox">
          {families.map(([family, cap]) => {
            const active = familyOf(spread.template) === family;
            const target = active ? spread.template : faceFor(family, index);
            return (
              <button
                key={family}
                role="option"
                aria-selected={active}
                className={"tpl-option" + (active ? " active" : "")}
                onClick={() => {
                  setOpen(false);
                  onPick(target);
                }}
              >
                <TemplateDiagram album={album} template={target} width={64} />
                <span className="tpl-option-name">{templateLabel(family)}</span>
                <span className="tpl-option-cap">
                  {cap > 1
                    ? t("gabarit.photos", { n: cap })
                    : t("gabarit.photos.une")}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** One template at miniature scale: real slots, real fold. */
function TemplateDiagram({
  album,
  template,
  width,
}: {
  album: Album;
  template: string;
  width: number;
}) {
  const canvas = mediaCanvas(album);
  const cap = templateCapacity(template);
  const rects = slotsFor(template, cap, canvas);
  const scale = width / canvas.w;
  return (
    <span
      className="tpl-diagram"
      style={{ width: canvas.w * scale, height: canvas.h * scale }}
      aria-hidden="true"
    >
      {rects.map((r, i) => (
        <span
          key={i}
          className="tpl-diagram-slot"
          style={{
            left: r.x * scale,
            top: r.y * scale,
            width: r.w * scale,
            height: r.h * scale,
          }}
        />
      ))}
      <span className="tpl-diagram-fold" />
    </span>
  );
}
