// Le choix d'une photo de la réserve (plan E, décision E3) : un popover de
// vignettes, la meilleure en premier et plus grande, la raison en une ligne
// sous chacune. « Ajouter une photo… » et « Remplacer par… » ouvrent le
// même choix ; ce qu'il fait à l'album vit dans `reserve.ts`.
//
// Le classement vient du moteur (`reserveClassee`) et peut prendre des
// secondes la première fois sur un gros dossier : le popover s'ouvre tout de
// suite et dit qu'il classe. Une planche pleine se dit avant le choix, avec
// la planche d'après en issue, plutôt qu'un refus après coup.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Candidat, Reserve } from "./bridge";
import { langue, t } from "./i18n";
import { placerMenu } from "./contextuel";
import { LazyThumb } from "./TriView";

/** Le libellé d'une raison : un code du moteur devient une ligne dans la
 *  langue de l'écran. Pour « proche », la date elle-même dit l'écart. */
export function libelleRaison(c: Candidat): string {
  if (c.raison === "meme_jour") return t("reserve.raison.meme_jour");
  if (c.raison === "proche" && c.taken) {
    const d = new Date(c.taken);
    if (!Number.isNaN(d.getTime())) {
      const date = new Intl.DateTimeFormat(langue(), {
        day: "numeric",
        month: "short",
        year: "numeric",
      }).format(d);
      return t("reserve.raison.proche", { date });
    }
  }
  return t("reserve.raison.nette");
}

export function ChoixReserve({
  point,
  reserve,
  pleine,
  onChoisir,
  onInsererApres,
  onFermer,
}: {
  point: { x: number; y: number };
  /** `null` tant que le moteur classe. */
  reserve: Reserve | null;
  /** La planche n'accueille pas une photo de plus, et pourquoi. */
  pleine: "target_full" | "target_text" | null;
  onChoisir: (c: Candidat) => void;
  onInsererApres: () => void;
  onFermer: () => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const options = useRef<(HTMLButtonElement | null)[]>([]);
  const [pos, setPos] = useState({ left: point.x, top: point.y });
  const [rang, setRang] = useState(0);
  const candidats = reserve?.candidats ?? [];

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
  }, [point.x, point.y, reserve, pleine]);

  useEffect(() => {
    const origine = document.activeElement as HTMLElement | null;
    return () => {
      origine?.focus?.();
    };
  }, []);

  // La première vignette prend le focus dès que la liste est là.
  useEffect(() => {
    options.current[rang]?.focus();
  }, [rang, candidats.length]);

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
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onDown, true);
    };
  }, [onFermer]);

  const surTouche = (e: React.KeyboardEvent) => {
    const n = candidats.length;
    if (!n) return;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") {
      e.preventDefault();
      setRang((r) => (r + 1) % n);
    } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
      e.preventDefault();
      setRang((r) => (r - 1 + n) % n);
    } else if (e.key === "Home") {
      e.preventDefault();
      setRang(0);
    } else if (e.key === "End") {
      e.preventDefault();
      setRang(n - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onChoisir(candidats[rang]);
    }
  };

  return (
    <div
      ref={root}
      className="choix-reserve"
      role="dialog"
      aria-label={t("reserve.titre")}
      style={{ left: `${pos.left}px`, top: `${pos.top}px` }}
      onKeyDown={surTouche}
      onContextMenu={(e) => e.preventDefault()}
    >
      <p className="choix-titre" title={reserve?.note}>
        {t("reserve.titre")}
      </p>
      {pleine && (
        <div className="choix-pleine">
          <p>{pleine === "target_text" ? t("reserve.page.texte") : t("reserve.pleine")}</p>
          <button type="button" className="link" onClick={onInsererApres}>
            {t("reserve.inserer.apres")}
          </button>
        </div>
      )}
      {reserve === null ? (
        <p className="choix-vide" role="status">
          {t("reserve.classement")}
        </p>
      ) : candidats.length === 0 ? (
        <p className="choix-vide">{t("reserve.vide")}</p>
      ) : (
        <div className="choix-liste" role="listbox" aria-label={t("reserve.titre")}>
          {candidats.map((c, i) => (
            <button
              key={c.src}
              ref={(el) => {
                options.current[i] = el;
              }}
              type="button"
              role="option"
              aria-selected={rang === i}
              aria-label={`${libelleRaison(c)}, ${c.src}`}
              className={"choix-option" + (i === 0 ? " premiere" : "") + (rang === i ? " vise" : "")}
              tabIndex={rang === i ? 0 : -1}
              onMouseEnter={() => setRang(i)}
              onClick={() => onChoisir(c)}
            >
              <span className="choix-vignette">
                <LazyThumb src={c.src} />
              </span>
              <span className="choix-raison">{libelleRaison(c)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
