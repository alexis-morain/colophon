// Le menu contextuel, en DOM (plan E, décision E1) : ouvert au clic droit sur
// une photo, le papier nu ou un objet libre, dans les deux rendus. Pas un
// menu natif Tauri, parce que le choix qui le suit porte des vignettes.
//
// Ce qu'il contient, où il s'ouvre et comment le clavier le parcourt vient de
// `contextuel.ts`, testé sans fenêtre. Ici : le rôle `menu` pour
// VoiceOver, le focus qui entre sur la première entrée et revient d'où il
// venait, Échap et le clic dehors qui referment avant que quoi que ce soit
// d'autre ne réagisse (la même capture que les sélecteurs), et un seul menu
// ouvert à la fois, ce que l'état d'`App` garantit.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { t } from "./i18n";
import { Entree, EntreeId, placerMenu, rangSuivant } from "./contextuel";

export function MenuContextuel({
  point,
  entrees,
  onChoisir,
  onFermer,
}: {
  /** Le pointeur au clic droit, en pixels de la fenêtre. */
  point: { x: number; y: number };
  entrees: Entree[];
  onChoisir: (id: EntreeId) => void;
  onFermer: () => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const boutons = useRef<(HTMLButtonElement | null)[]>([]);
  const [pos, setPos] = useState({ left: point.x, top: point.y });
  const [rang, setRang] = useState<number | null>(null);
  const libelles = entrees.map((e) => t(e.cle));
  const actives = entrees.map((e) => !e.desactive);

  // Mesuré une fois posé, puis rabattu dans la fenêtre : un menu ouvert près
  // du bord droit se décale, il ne se coupe pas.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos(
      placerMenu(
        point,
        { w: r.width, h: r.height },
        { w: window.innerWidth, h: window.innerHeight },
      ),
    );
  }, [point.x, point.y]);

  // Le focus entre sur la première entrée active, et revient d'où il venait
  // à la fermeture : un menu ne laisse pas le clavier dans le vide.
  useEffect(() => {
    const origine = document.activeElement as HTMLElement | null;
    setRang(rangSuivant(null, "ArrowDown", libelles, actives));
    return () => {
      origine?.focus?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (rang !== null) boutons.current[rang]?.focus();
  }, [rang]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      e.preventDefault();
      onFermer();
    };
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) onFermer();
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("blur", onFermer);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("blur", onFermer);
    };
  }, [onFermer]);

  const surTouche = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (rang !== null && actives[rang]) onChoisir(entrees[rang].id);
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      onFermer();
      return;
    }
    const suivant = rangSuivant(rang, e.key, libelles, actives);
    if (suivant !== null) {
      e.preventDefault();
      setRang(suivant);
    }
  };

  return (
    <div
      ref={root}
      className="menu-ctx"
      role="menu"
      aria-label={t("ctx.titre")}
      style={{ left: `${pos.left}px`, top: `${pos.top}px` }}
      onKeyDown={surTouche}
      onContextMenu={(e) => e.preventDefault()}
    >
      {entrees.map((e, i) => (
        <button
          key={e.id}
          ref={(el) => {
            boutons.current[i] = el;
          }}
          type="button"
          role="menuitem"
          className={"menu-ctx-item" + (rang === i ? " vise" : "")}
          tabIndex={-1}
          disabled={Boolean(e.desactive)}
          aria-disabled={Boolean(e.desactive)}
          // Le nom reste le libellé ; l'infobulle dit pourquoi l'entrée dort.
          aria-label={libelles[i]}
          title={e.desactive ? t(e.desactive) : undefined}
          onMouseEnter={() => actives[i] && setRang(i)}
          onClick={() => onChoisir(e.id)}
        >
          {libelles[i]}
        </button>
      ))}
    </div>
  );
}
