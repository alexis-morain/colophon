// The template picker, drawn instead of named: each choice is a miniature
// spread in the album's own geometry (slotsFor, the engine's arithmetic), so
// picking a layout means seeing it. The engine's template names stay in the
// data; the interface speaks French.

import { useEffect, useRef, useState } from "react";
import { Album, mediaCanvas, slotsFor, Spread, templateCapacity } from "./album";
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

export function templateLabel(template: string): string {
  const verso = template.endsWith("_verso");
  const family = verso ? template.slice(0, -"_verso".length) : template;
  const base = FAMILY_LABELS[family] ?? family;
  return verso ? `${base} · verso` : base;
}

export function TemplatePicker({
  album,
  spread,
  onPick,
}: {
  album: Album;
  spread: Spread;
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

  const choices = templateChoices(spread);

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
          {choices.map(([t, cap]) => (
            <button
              key={t}
              role="option"
              aria-selected={t === spread.template}
              className={"tpl-option" + (t === spread.template ? " active" : "")}
              onClick={() => {
                setOpen(false);
                onPick(t);
              }}
            >
              <TemplateDiagram album={album} template={t} width={76} />
              <span className="tpl-option-name">{templateLabel(t)}</span>
              <span className="tpl-option-cap">
                {cap} photo{cap > 1 ? "s" : ""}
              </span>
            </button>
          ))}
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
