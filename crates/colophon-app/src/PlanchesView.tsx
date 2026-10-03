// The light table: every spread as a miniature, on one scrollable grid.
// Reordering is a drag, duplicating a keystroke, a photo-less or text
// spread one click away: the sequencing work that makes or breaks the book
// happens here, nearly for free. Images only load when their cell scrolls
// into view; the blob pool stays bounded.
//
// **Le glisser passe par le pointeur, jamais par le glisser HTML5.** Celui-ci
// dépend de ce que le webview veut bien livrer, et ne se vérifie pas au
// harnais. Un appui devient un glisser au-delà de six pixels (en deçà, c'est
// le clic d'avant) ; la figure suit la main par `transform`, jamais par le
// layout, et la cible est la cellule dont le rectangle contient le pointeur
// (`planches.ts::cibleSous`). Rien ne s'écrit avant le relâchement, et Échap
// annule sans rien écrire.
//
// **Et elle se parcourt au clavier**, parce que c'est un écran de
// navigation : c'est ici qu'on cherche une planche, et une grille qu'on ne
// peut atteindre qu'à la souris n'est pas une grille, c'est une image. Un
// seul arrêt de tabulation — la cellule courante — puis les flèches à
// l'intérieur, les verticales d'une rangée entière. La couverture est la
// page zéro et se parcourt avec le reste ; ce qu'elle porte dedans attend
// la session qui lui donnera une scène.

import { useEffect, useRef, useState } from "react";
import { t } from "./i18n";
import { Album, Spread, spreadGeometry, slotsFor } from "./album";
import { badgesDe, useReleve } from "./photos";
import { reglagePose, useReglages } from "./reglages";
import { thumbCropStyle } from "./SpreadView";
import { cachedThumb, loadThumb } from "./thumbs";
import { cibleSous, RectCellule, seuilFranchi } from "./planches";

/** Un appui sur une planche, qui deviendra peut-être un glisser. */
type Appui = {
  from: number;
  pointeur: number;
  x0: number;
  y0: number;
  /** Les cellules mesurées au début du glisser : seule la figure tenue
   *  bouge, par `transform`, donc la grille ne change pas sous la main. */
  rects: RectCellule[] | null;
  cible: number | null;
};

export function PlanchesView({
  album,
  current,
  onSelect,
  onOpen,
  onMove,
  onLock,
}: {
  album: Album;
  /** Index of the highlighted cell; -1 is the cover, page zero. */
  current: number;
  onSelect: (at: number) => void;
  /** Double-click: open this spread (or the cover, -1) in the book view. */
  onOpen: (at: number) => void;
  onMove: (from: number, to: number) => void;
  onLock: (at: number) => void;
}) {
  const [dropAt, setDropAt] = useState<number | null>(null);
  const [tenue, setTenue] = useState<{ at: number; dx: number; dy: number } | null>(null);
  const appui = useRef<Appui | null>(null);
  // Le clic qui suit le relâchement d'un glisser n'est pas une sélection.
  const avaleClic = useRef(false);
  const grille = useRef<HTMLDivElement>(null);

  const lacher = () => {
    appui.current = null;
    setTenue(null);
    setDropAt(null);
  };

  // Échap pendant un glisser annule sans rien écrire. En capture, et arrêtée
  // là : la table clavier d'App ne doit pas la lire une seconde fois.
  const glisse = tenue !== null;
  useEffect(() => {
    if (!glisse) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      avaleClic.current = true;
      lacher();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [glisse]);

  const surAppui = (e: React.PointerEvent<HTMLElement>, at: number) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    avaleClic.current = false;
    appui.current = {
      from: at,
      pointeur: e.pointerId,
      x0: e.clientX,
      y0: e.clientY,
      rects: null,
      cible: null,
    };
  };

  const surMouvement = (e: React.PointerEvent<HTMLElement>) => {
    const a = appui.current;
    if (!a || a.pointeur !== e.pointerId) return;
    const dx = e.clientX - a.x0;
    const dy = e.clientY - a.y0;
    if (!a.rects) {
      if (!seuilFranchi(dx, dy)) return;
      e.currentTarget.setPointerCapture(e.pointerId);
      a.rects = [...(grille.current?.querySelectorAll<HTMLElement>("[data-at]") ?? [])]
        .map((el) => {
          const r = el.getBoundingClientRect();
          return {
            at: Number(el.dataset.at),
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
          };
        })
        .filter((r) => r.at !== a.from);
    }
    a.cible = cibleSous(a.rects, e.clientX, e.clientY);
    setTenue({ at: a.from, dx, dy });
    setDropAt(a.cible);
  };

  const surRelache = (e: React.PointerEvent<HTMLElement>) => {
    const a = appui.current;
    if (!a || a.pointeur !== e.pointerId) return;
    if (a.rects) {
      avaleClic.current = true;
      if (a.cible !== null && a.cible !== a.from) onMove(a.from, a.cible);
    }
    lacher();
  };

  /** Combien de cellules par rangée, tel que la grille les pose vraiment :
   *  une flèche verticale doit sauter une rangée, pas un nombre inventé. */
  const colonnes = () => {
    const el = grille.current;
    if (!el) return 1;
    const pistes = getComputedStyle(el)
      .gridTemplateColumns.split(" ")
      .filter(Boolean).length;
    return Math.max(1, pistes);
  };

  // Le clavier suit la cellule courante — mais seulement s'il est déjà dans
  // la grille : sinon la vue volerait le focus à la barre ou au pied.
  useEffect(() => {
    const el = grille.current;
    if (!el || !el.contains(document.activeElement)) return;
    el.querySelector<HTMLElement>(`[data-at="${current}"]`)?.focus();
  }, [current]);

  const surTouche = (e: React.KeyboardEvent, at: number) => {
    const dernier = album.spreads.length - 1;
    // Arrêtée ici, la touche n'atteint pas la table de commandes d'App, qui
    // ferait le même pas une seconde fois.
    const aller = (to: number) => {
      e.preventDefault();
      e.stopPropagation();
      onSelect(Math.min(dernier, Math.max(-1, to)));
    };
    // ⌥ flèche déplace la planche elle-même : le livre se réordonne au
    // clavier comme à la souris, et le focus suit la planche déplacée.
    if (e.altKey && at >= 0) {
      const vers: Record<string, number> = {
        ArrowRight: at + 1,
        ArrowLeft: at - 1,
        ArrowDown: at + colonnes(),
        ArrowUp: at - colonnes(),
      };
      const to = vers[e.key];
      if (to !== undefined) {
        e.preventDefault();
        e.stopPropagation();
        const borne = Math.min(dernier, Math.max(0, to));
        if (borne !== at) onMove(at, borne);
        return;
      }
    }
    switch (e.key) {
      case "ArrowRight":
        aller(at + 1);
        break;
      case "ArrowLeft":
        aller(at - 1);
        break;
      case "ArrowDown":
        aller(at + colonnes());
        break;
      case "ArrowUp":
        aller(at - colonnes());
        break;
      case "Home":
        aller(-1);
        break;
      case "End":
        aller(dernier);
        break;
      // Entrée et Espace ouvrent : la cellule se comporte comme le bouton
      // qu'elle est pour qui ne la voit pas.
      case "Enter":
      case " ":
        e.preventDefault();
        e.stopPropagation();
        onOpen(at);
        break;
    }
  };

  return (
    <div className="planches" role="list" aria-label={t("table.liste")} ref={grille}>
      <CoverCell
        album={album}
        current={current === -1}
        onSelect={() => onSelect(-1)}
        onOpen={() => onOpen(-1)}
        onKey={(e) => surTouche(e, -1)}
      />
      {album.spreads.map((spread, i) => (
        <PlancheCell
          key={i}
          album={album}
          spread={spread}
          index={i}
          current={i === current}
          dropping={dropAt === i}
          onSelect={() => onSelect(i)}
          onOpen={() => onOpen(i)}
          onLock={() => onLock(i)}
          onKey={(e) => surTouche(e, i)}
          tenue={tenue?.at === i ? tenue : null}
          onAppui={(e) => surAppui(e, i)}
          onMouvement={surMouvement}
          onRelache={surRelache}
          onAnnule={lacher}
          avaleClic={() => {
            const avale = avaleClic.current;
            avaleClic.current = false;
            return avale;
          }}
        />
      ))}
    </div>
  );
}

function PlancheCell({
  album,
  spread,
  index,
  current,
  dropping,
  onSelect,
  onOpen,
  onLock,
  onKey,
  tenue,
  onAppui,
  onMouvement,
  onRelache,
  onAnnule,
  avaleClic,
}: {
  album: Album;
  spread: Spread;
  index: number;
  current: boolean;
  dropping: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onLock: () => void;
  onKey: (e: React.KeyboardEvent) => void;
  /** Le décalage de la figure quand c'est elle qu'on glisse. */
  tenue: { dx: number; dy: number } | null;
  onAppui: (e: React.PointerEvent<HTMLElement>) => void;
  onMouvement: (e: React.PointerEvent<HTMLElement>) => void;
  onRelache: (e: React.PointerEvent<HTMLElement>) => void;
  onAnnule: () => void;
  /** Vrai une fois si le clic qui arrive suit un glisser. */
  avaleClic: () => boolean;
}) {
  // Les cases en alerte, par la règle de la planche (`badgesDe`) : ce que le
  // relevé sait tout de suite, et « sombre » quand la vignette d'une case
  // s'est décodée, puisqu'il lit ses pixels.
  const releve = useReleve();
  const [sombres, setSombres] = useState<ReadonlySet<number>>(() => new Set());
  const rects = slotsFor(spread.template, spread.slots.length, spreadGeometry(album));
  const enAlerte = spread.slots.filter((slot, i) => {
    const r = rects[i];
    if (!r) return false;
    const b = badgesDe(slot.src, null, r, 1, slot.zoom ?? 1, undefined, releve);
    return b.alertes.length > 0 || sombres.has(i);
  }).length;
  const surSombre = (i: number, sombre: boolean) =>
    setSombres((avant) => {
      if (avant.has(i) === sombre) return avant;
      const apres = new Set(avant);
      if (sombre) apres.add(i);
      else apres.delete(i);
      return apres;
    });
  const ditAlerte =
    enAlerte === 0
      ? ""
      : enAlerte === 1
        ? t("table.alerte.une")
        : t("table.alerte", { n: enAlerte });
  return (
    <figure
      role="listitem"
      aria-roledescription={t("table.cellule.role")}
      className={
        "planche-cell" +
        (current ? " current" : "") +
        (dropping ? " dropping" : "") +
        (tenue ? " tenue" : "")
      }
      style={tenue ? { transform: `translate(${tenue.dx}px, ${tenue.dy}px)` } : undefined}
      onPointerDown={onAppui}
      onPointerMove={onMouvement}
      onPointerUp={onRelache}
      onPointerCancel={onAnnule}
      // Une vignette se glisserait d'elle-même, en HTML5, et le navigateur
      // annulerait alors le pointeur au milieu du geste.
      onDragStart={(e) => e.preventDefault()}
      onClick={(e) => {
        e.stopPropagation();
        if (avaleClic()) return;
        onSelect();
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
      title={
        (spread.caption ? `${spread.caption} · ` : "") +
        t("table.cellule.titre", { n: index + 1 })
      }
      // Un seul arrêt de tabulation pour toute la grille, celui de la
      // cellule courante : les flèches font le reste, et Tab ne devient pas
      // soixante pressions pour traverser un album.
      data-at={index}
      tabIndex={current ? 0 : -1}
      aria-current={current ? true : undefined}
      aria-label={
        (spread.caption ? `${spread.caption} · ` : "") +
        t("table.cellule.nom", { n: index + 1 }) +
        (ditAlerte ? ` · ${ditAlerte}` : "")
      }
      onKeyDown={onKey}
    >
      <MiniSpread album={album} spread={spread} onSombre={surSombre} />
      <figcaption className="planche-meta">
        <span className="planche-num">{index + 1}</span>
        {spread.caption && <span className="planche-chapter">{spread.caption}</span>}
        <span className="planche-flags">
          {spread.edited && (
            <span
              className="badge-edited"
              title={t("table.editee")}
            />
          )}
          {ditAlerte && <span className="badge-alerte" title={ditAlerte} />}
          <button
            className={"lock" + (spread.locked ? " locked" : "")}
            onClick={(e) => {
              e.stopPropagation();
              onLock();
            }}
            title={spread.locked ? t("table.figee") : t("table.figer")}
            aria-pressed={spread.locked ?? false}
          >
            <LockGlyph open={!spread.locked} />
          </button>
        </span>
      </figcaption>
    </figure>
  );
}

/**
 * The cover as the light table's page zero: the whole book starts here, so
 * the whole book shows here. Front panel only, at the page's own aspect;
 * it neither drags nor receives drops, a cover has one possible place.
 */
function CoverCell({
  album,
  current,
  onSelect,
  onOpen,
  onKey,
}: {
  album: Album;
  current: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onKey: (e: React.KeyboardEvent) => void;
}) {
  const cover = album.cover ?? { title: album.title };
  return (
    <figure
      role="listitem"
      className={"planche-cell planche-cover" + (current ? " current" : "")}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
      title={t("table.couverture.titre")}
      data-at={-1}
      tabIndex={current ? 0 : -1}
      aria-current={current ? true : undefined}
      aria-label={t("table.couverture")}
      onKeyDown={onKey}
    >
      <div
        className="mini-cover"
        style={{ aspectRatio: `${album.trim_mm.w} / ${album.trim_mm.h}` }}
      >
        {cover.photo && <MiniImg slot={cover.photo} />}
        <span className="mini-cover-title">{cover.title || album.title}</span>
      </div>
      <figcaption className="planche-meta">
        <span className="planche-num">C</span>
        <span className="planche-chapter">{t("table.couverture")}</span>
      </figcaption>
    </figure>
  );
}

/** A minimal padlock, drawn rather than emoji'd. */
export function LockGlyph({ open }: { open: boolean }) {
  return (
    <svg viewBox="0 0 10 12" width="10" height="12" aria-hidden="true">
      <rect x="1" y="5.5" width="8" height="6" fill="currentColor" />
      <path
        d={open ? "M 2.8 5.5 V 3.2 A 2.2 2.2 0 0 1 7.2 3.2 V 4.2" : "M 2.8 5.5 V 3.2 A 2.2 2.2 0 0 1 7.2 3.2 V 5.5"}
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        transform={open ? "translate(1.6 -1.4) rotate(24 5 3.5)" : undefined}
      />
    </svg>
  );
}

/**
 * One spread at postage size: real geometry, real crops, images gated by
 * an IntersectionObserver so sixty planches cost only what is on screen.
 */
function MiniSpread({
  album,
  spread,
  onSombre,
}: {
  album: Album;
  spread: Spread;
  /** Une vignette décodée dit si sa case est sombre. */
  onSombre: (cell: number, sombre: boolean) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const geom = spreadGeometry(album);
  const rects = slotsFor(spread.template, spread.slots.length, geom);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisible(true);
          io.disconnect();
        }
      },
      { rootMargin: "300px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      className="mini-spread"
      style={{ aspectRatio: `${geom.w} / ${geom.h}` }}
    >
      {spread.slots.map((slot, i) => {
        const r = rects[i];
        if (!r) return null;
        return (
          <span
            key={i}
            className="mini-slot"
            style={{
              left: `${(r.x / geom.w) * 100}%`,
              top: `${(r.y / geom.h) * 100}%`,
              width: `${(r.w / geom.w) * 100}%`,
              height: `${(r.h / geom.h) * 100}%`,
            }}
          >
            {visible && (
              <MiniImg slot={slot} rect={r} onSombre={(v) => onSombre(i, v)} />
            )}
          </span>
        );
      })}
      {/* Les planches de texte se lisent dans la grille : sans ça, la garde
          et le colophon y sont deux rectangles vides. */}
      {spread.text !== undefined && (
        <span className="mini-text" aria-hidden="true">
          {spread.text.trim() ? spread.text : "texte"}
        </span>
      )}
      <span className="mini-fold" aria-hidden="true" />
    </div>
  );
}

function MiniImg({
  slot,
  rect,
  onSombre,
}: {
  slot: { src: string; focal: [number, number]; zoom?: number };
  /** La case, pour une planche ; la couverture n'en passe pas, son
   *  alerte n'est pas de ce chantier. */
  rect?: { x: number; y: number; w: number; h: number };
  onSombre?: (sombre: boolean) => void;
}) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(slot.src));
  // thumbCropStyle carries the photo's adjustment: follow it.
  useReglages();
  const img = useRef<HTMLImageElement>(null);
  const releve = useReleve();
  // « Sombre » lit les pixels au travers du réglage posé, comme sur la
  // planche ; la clé relance la lecture quand le réglage change.
  const r = reglagePose(slot.src);
  const cle = `${r?.expo ?? 0}|${r?.contraste ?? 0}|${r?.nb ?? false}`;
  useEffect(() => {
    const el = img.current;
    if (!el || !url || !rect || !onSombre) return;
    const lire = () => {
      if (!el.naturalWidth) return;
      const b = badgesDe(slot.src, el, rect, 1, slot.zoom ?? 1, reglagePose(slot.src), releve);
      onSombre(b.alertes.some((a) => a.code === "sombre"));
    };
    if (el.complete) {
      lire();
      return;
    }
    el.addEventListener("load", lire, { once: true });
    return () => el.removeEventListener("load", lire);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url, releve, cle, slot.src]);
  useEffect(() => {
    let alive = true;
    if (!cachedThumb(slot.src)) setUrl(undefined);
    loadThumb(slot.src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [slot.src]);
  return url ? <img ref={img} src={url} alt="" style={thumbCropStyle(slot)} /> : null;
}
