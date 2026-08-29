// The template picker, drawn instead of named: each choice is a miniature
// spread in the album's own geometry (slotsFor, the engine's arithmetic), so
// picking a layout means seeing it. The engine's template names stay in the
// data; the interface speaks French.
//
// What it offers is one entry per *arrangement* — how many cells on each
// page, in how many rows — and not one per template: the offered catalogue
// holds up to 171 templates a four-photo spread can take, and most of that
// number is the same layout twice over, once with an 8 mm caption band and
// once with another cell shape. `gabarit.ts` does the folding and the
// naming; the variant this picker actually applies is the one these photos
// fit best, judged by the engine (`gabarit::trahison`) and never here.
//
// Recto/verso variants are one choice here too: the picker shows the
// arrangement and flips it onto the right page from the spread's parity,
// the way the Composer does (`layout.rs::with_flip`). The engine's fallback
// table is untouched; this is a display filter only.

import { useEffect, useMemo, useRef, useState } from "react";
import { Album, spreadGeometry, slotsFor, Spread, templateCapacity } from "./album";
import { gabaritsCompatibles } from "./bridge";
import { templateChoices } from "./edits";
import { t } from "./i18n";
import {
  Choix,
  choixOfferts,
  cleDeForme,
  faceFor,
  formeDe,
  libelleGabarit,
} from "./gabarit";

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
  // The compatible template names and their betrayal, asked to the engine
  // when the panel opens; null while it answers (or when it cannot), and
  // then the count-compatible list stands in: an honest picker beats an
  // empty one.
  const [compat, setCompat] = useState<[string, number][] | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    let dead = false;
    setCompat(null);
    void gabaritsCompatibles(spread.slots.map((s) => s.src)).then((notes) => {
      if (!dead && notes) setCompat(notes);
    });
    return () => {
      dead = true;
    };
  }, [open, spread]);

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

  // One entry per arrangement, grouped by how many photos it holds: the
  // groups below the spread's own count are the layouts that drop a photo,
  // and a heading is what makes that visible instead of surprising.
  const groupes = useMemo(() => {
    const notes: [string, number][] =
      compat ?? templateChoices(spread).map(([nom]) => [nom, 1]);
    const par = new Map<number, Choix[]>();
    for (const c of choixOfferts(notes, spread.template)) {
      const liste = par.get(c.capacite);
      if (liste) liste.push(c);
      else par.set(c.capacite, [c]);
    }
    return [...par.entries()].sort((x, y) => y[0] - x[0]);
  }, [compat, spread]);

  const formeCourante = formeDe(spread.template);
  const cleCourante = formeCourante ? cleDeForme(formeCourante) : "";

  return (
    <div className="tpl" ref={root}>
      <button
        className="tpl-current"
        onClick={() => setOpen((o) => !o)}
        title={t("gabarit.titre")}
        aria-expanded={open}
      >
        <TemplateDiagram album={album} template={spread.template} width={34} />
        <span>{libelleGabarit(spread.template)}</span>
      </button>
      {open && (
        <div className="tpl-panel" role="listbox">
          {groupes.map(([cap, choix]) => {
            const titre =
              cap > 1 ? t("gabarit.photos", { n: cap }) : t("gabarit.photos.une");
            return (
              <div className="tpl-groupe" key={cap} role="group" aria-label={titre}>
                <h4 className="tpl-groupe-nom">{titre}</h4>
                <div className="tpl-groupe-cases">
                  {choix.map((c) => {
                    const active = c.cle === cleCourante;
                    const target = active
                      ? spread.template
                      : faceFor(c.template, index);
                    return (
                      <button
                        key={c.cle}
                        role="option"
                        aria-selected={active}
                        className={"tpl-option" + (active ? " active" : "")}
                        onClick={() => {
                          setOpen(false);
                          onPick(target);
                        }}
                      >
                        <TemplateDiagram
                          album={album}
                          template={target}
                          width={64}
                        />
                        <span className="tpl-option-name">{c.libelle}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
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
  const geom = spreadGeometry(album);
  const cap = templateCapacity(template);
  const rects = slotsFor(template, cap, geom);
  const scale = width / geom.w;
  return (
    <span
      className="tpl-diagram"
      style={{ width: geom.w * scale, height: geom.h * scale }}
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
