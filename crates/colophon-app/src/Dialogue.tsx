// The one modal shell of the app. Six panels used to draw the same overlay
// each on its own — a dimmed backdrop, a panel, a header with a close link —
// and none of them was a dialog to anyone who could not see it: no role, no
// modal flag, no focus, and the editor behind still reachable by Tab. Under
// VoiceOver ⌘, opened the preferences and announced nothing.
//
// This shell gives every panel the same contract, once:
// - `role="dialog"`, `aria-modal`, named by its heading;
// - the focus moves into the panel when it opens, and returns where it was
//   when it closes;
// - Tab cycles inside the panel and never reaches the editor;
// - everything else in the window is `inert` while the panel is up: the
//   pointer, the keyboard and the screen reader all see one thing.
//
// Escape stays with App's keyboard table, where every panel already has its
// line, because the table decides what Escape means when two things are up
// (the end-of-build report over the shortcuts, say). A click on the backdrop
// closes, a click in the panel does not.

import { useEffect, useLayoutEffect, useRef } from "react";
import { t } from "./i18n";

export function Dialogue({
  titre,
  onClose,
  className,
  panelClassName,
  children,
}: {
  /** The heading shown at the top, and the dialog's accessible name. */
  titre: string;
  onClose: () => void;
  /** Class of the backdrop, `raccourcis` by default. */
  className?: string;
  /** Class of the panel, `raccourcis-panel` plus whatever the caller adds. */
  panelClassName?: string;
  children: React.ReactNode;
}) {
  const fond = useRef<HTMLDivElement>(null);
  const panneau = useRef<HTMLDivElement>(null);
  const titreId = useRef(`dialogue-${Math.random().toString(36).slice(2, 9)}`);

  // Everything beside the backdrop goes inert for the panel's lifetime, and
  // only what this panel made inert comes back — a second panel over the
  // first (the shortcuts over the preferences) must not wake the editor when
  // it closes.
  useLayoutEffect(() => {
    const el = fond.current;
    const parent = el?.parentElement;
    if (!el || !parent) return;
    const muets: Element[] = [];
    for (const frere of Array.from(parent.children)) {
      if (frere === el || frere.hasAttribute("inert")) continue;
      frere.setAttribute("inert", "");
      muets.push(frere);
    }
    return () => {
      for (const frere of muets) frere.removeAttribute("inert");
    };
  }, []);

  // Focus in on open, back out on close.
  useEffect(() => {
    const avant = document.activeElement as HTMLElement | null;
    panneau.current?.focus();
    return () => {
      if (avant && document.contains(avant)) avant.focus();
    };
  }, []);

  // Tab cycles inside the panel. The inert siblings already keep the editor
  // out; this keeps the focus from leaving to the window's chrome.
  const surTab = (e: React.KeyboardEvent) => {
    if (e.key !== "Tab" || !panneau.current) return;
    const focusables = Array.from(
      panneau.current.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((n) => n.offsetParent !== null || n === document.activeElement);
    if (focusables.length === 0) {
      e.preventDefault();
      return;
    }
    const premier = focusables[0];
    const dernier = focusables[focusables.length - 1];
    const actif = document.activeElement;
    if (e.shiftKey && (actif === premier || actif === panneau.current)) {
      e.preventDefault();
      dernier.focus();
    } else if (!e.shiftKey && actif === dernier) {
      e.preventDefault();
      premier.focus();
    }
  };

  return (
    <div ref={fond} className={className ?? "raccourcis"} onClick={onClose}>
      <div
        ref={panneau}
        className={["raccourcis-panel", panelClassName].filter(Boolean).join(" ")}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titreId.current}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={surTab}
      >
        <header className="raccourcis-head">
          <h2 id={titreId.current}>{titre}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>
        {children}
      </div>
    </div>
  );
}
