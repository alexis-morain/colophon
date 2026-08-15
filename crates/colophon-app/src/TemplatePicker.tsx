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

/** French labels for template families; the diagram carries the rest. */
const FAMILY_LABELS: Record<string, string> = {
  full1: "Pleine page",
  solo: "Une photo",
  solo_paysage: "Une photo, paysage",
  solo_pano: "Une photo, panorama",
  solo_etroit: "Une photo, étroite",
  solo_carre: "Une photo, carrée",
  duo: "Deux photos",
  duo_portrait: "Deux portraits",
  duo_paysage: "Deux paysages",
  duo_etroit: "Deux photos, étroites",
  duo_pano: "Deux panoramas",
  trio: "Trois photos",
  trio_portrait: "Trois photos, portraits",
  quad: "Quatre photos",
  quad_portrait: "Quatre portraits",
  quad_etroit: "Quatre photos, étroites",
  quad_pano: "Quatre panoramas",
  six: "Six photos",
  octo: "Huit photos",
};

/** The family behind a template name, verso suffix folded away. */
function familyOf(template: string): string {
  return template.endsWith("_verso")
    ? template.slice(0, -"_verso".length)
    : template;
}

export function templateLabel(template: string): string {
  const family = familyOf(template);
  return FAMILY_LABELS[family] ?? family;
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
        title="Gabarit de la planche"
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
                  {cap} photo{cap > 1 ? "s" : ""}
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
