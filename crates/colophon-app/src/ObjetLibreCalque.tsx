// La prise sur un objet libre : le cadre, les poignées, et les trois gestes.
//
// **Du DOM, dans les deux rendus.** Le précédent est écrit dans `SpreadView` :
// les pastilles d'une case restent en DOM au-dessus du canvas, parce qu'« une
// infobulle porte le remède, et un canvas n'a pas d'infobulle ». Une poignée
// est du même bois — elle a un curseur, un nom pour VoiceOver, un état de
// survol —, et la peindre deux fois pour la même image serait deux fois le
// travail et deux occasions de diverger.
//
// **Le calque ne décide de rien de géométrique.** Ce qui dit qu'une boîte a le
// droit d'être là vit dans `scene.ts` : `retenirAuPli` pour la butée dure,
// `horsMarge` pour l'avertissement mou. Ici on ne fait que suivre la main et
// appeler ces deux-là.
//
// **Un geste ne pose qu'un pas d'annulation.** Pendant le geste, un brouillon
// que le rendu affiche ; au relâchement, une seule édition d'album. C'est la
// règle du recadrage et des glissières de réglage, mot pour mot.

import { useRef } from "react";
import { Rect, SpreadGeometry } from "./album";
import { t } from "./i18n";
import { angleEcran, centre, Cote, coteDe, horsMarge, retenirAuPli, tourner } from "./scene";

/** La boîte et l'angle qu'un geste est en train de rendre. */
export type PoseObjet = { rect: Rect; angle: number };

/** Ce qu'une poignée tient, et donc quel coin reste fixe. */
const COINS = [
  { cle: "hg", fixe: 2, gauche: true, haut: true },
  { cle: "hd", fixe: 3, gauche: false, haut: true },
  { cle: "bd", fixe: 0, gauche: false, haut: false },
  { cle: "bg", fixe: 1, gauche: true, haut: false },
] as const;

/** Plus petit qu'une ligne de corps 6, une boîte n'est plus saisissable. */
const MIN_MM = 4;

export function ObjetLibreCalque({
  pose,
  geom,
  mm,
  deborde,
  onDraft,
  onCommit,
  onEcrire,
  onSupprimer,
}: {
  /** La boîte affichée, repère de l'écran : la posée, ou celle du geste. */
  pose: PoseObjet;
  geom: SpreadGeometry;
  mm: number;
  /** Le texte dépasse le bas de sa boîte : dit ici, jamais coupé. */
  deborde: boolean;
  onDraft: (p: PoseObjet | null) => void;
  onCommit: (p: PoseObjet) => void;
  onEcrire: () => void;
  onSupprimer: () => void;
}) {
  const calque = useRef<HTMLDivElement>(null);
  const geste = useRef<{
    mode: "deplacer" | "tailler" | "tourner";
    coin: number;
    depart: { x: number; y: number };
    /** Le centre de la boîte en pixels client, pris au début du geste. Une
     *  rotation autour du centre laisse le centre de la boîte englobante en
     *  place, donc `getBoundingClientRect` le donne même sur un objet
     *  tourné. */
    centre: { x: number; y: number };
    pose: PoseObjet;
    cote: Cote;
    bouge: boolean;
    /** La dernière pose calculée. C'est **elle** qu'on valide au
     *  relâchement, jamais la prop : un dernier déplacement et le
     *  relâchement peuvent tomber dans la même image, et la prop serait
     *  alors d'une image en retard. */
    dernier: PoseObjet | null;
  } | null>(null);

  const { rect, angle } = pose;
  const hors = horsMarge(rect, angle, geom);

  const prendre =
    (mode: "deplacer" | "tailler" | "tourner", coin: number) =>
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      e.stopPropagation();
      e.currentTarget.setPointerCapture(e.pointerId);
      const b = calque.current?.getBoundingClientRect();
      geste.current = {
        mode,
        coin,
        depart: { x: e.clientX, y: e.clientY },
        centre: b
          ? { x: b.left + b.width / 2, y: b.top + b.height / 2 }
          : { x: e.clientX, y: e.clientY },
        pose,
        // Le côté est pris **au début** du geste et gardé jusqu'à sa fin :
        // sans ça un glissement ferait sauter l'objet d'une page à l'autre au
        // moment où il touche le pli.
        cote: coteDe(rect, geom),
        bouge: false,
        dernier: null,
      };
    };

  const suivre = (e: React.PointerEvent) => {
    const g = geste.current;
    if (!g) return;
    const dx = (e.clientX - g.depart.x) / mm;
    const dy = (e.clientY - g.depart.y) / mm;
    if (!g.bouge && Math.abs(dx) + Math.abs(dy) < 0.3) return;
    g.bouge = true;

    let suivante: PoseObjet;
    if (g.mode === "deplacer") {
      suivante = {
        angle: g.pose.angle,
        rect: { ...g.pose.rect, x: g.pose.rect.x + dx, y: g.pose.rect.y + dy },
      };
    } else if (g.mode === "tourner") {
      suivante = { rect: g.pose.rect, angle: tourne(g, e) };
    } else {
      suivante = tailler(g.pose, g.coin, dx, dy);
    }

    // Le pli est dur : on bute, on ne refuse pas — le geste continue de
    // suivre la main.
    suivante = {
      ...suivante,
      rect: retenirAuPli(suivante.rect, suivante.angle, geom, g.cote),
    };
    g.dernier = suivante;
    onDraft(suivante);
  };

  const lacher = (e: React.PointerEvent) => {
    const g = geste.current;
    geste.current = null;
    if (!g) return;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
    if (g.bouge && g.dernier) onCommit(g.dernier);
    onDraft(null);
  };

  const poignee = (i: number) => {
    const c = COINS[i];
    return (
      <button
        key={c.cle}
        type="button"
        className={`objet-poignee objet-poignee-${c.cle}`}
        aria-label={t("objet.tailler")}
        onPointerDown={prendre("tailler", i)}
        onPointerMove={suivre}
        onPointerUp={lacher}
        onPointerCancel={lacher}
      />
    );
  };

  return (
    <div
      ref={calque}
      className={"objet-calque" + (hors ? " hors-marge" : "")}
      style={{
        left: `${rect.x * mm}px`,
        top: `${rect.y * mm}px`,
        width: `${rect.w * mm}px`,
        height: `${rect.h * mm}px`,
        transform: angle === 0 ? undefined : `rotate(${angleEcran(angle)}deg)`,
      }}
      onPointerDown={prendre("deplacer", -1)}
      onPointerMove={suivre}
      onPointerUp={lacher}
      onPointerCancel={lacher}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onEcrire();
      }}
      onClick={(e) => e.stopPropagation()}
    >
      {COINS.map((_, i) => poignee(i))}
      <button
        type="button"
        className="objet-poignee objet-poignee-tourner"
        aria-label={t("objet.tourner")}
        onPointerDown={prendre("tourner", -1)}
        onPointerMove={suivre}
        onPointerUp={lacher}
        onPointerCancel={lacher}
      />
      {(hors || deborde) && (
        <span
          className="objet-avertis"
          // Les pastilles et la croix appartiennent à l'objet, donc elles le
          // suivent — mais elles se lisent, donc elles se redressent. Une
          // contre-rotation autour de leur propre centre, et rien d'autre.
          style={{ transform: angle === 0 ? undefined : `rotate(${angle}deg)` }}
        >
          {hors && <span className="objet-warn" title={t("objet.marge")}>⌖</span>}
          {deborde && <span className="objet-warn" title={t("objet.deborde")}>↧</span>}
        </span>
      )}
      <button
        type="button"
        className="objet-supprimer"
        style={{ transform: angle === 0 ? undefined : `rotate(${angle}deg)` }}
        aria-label={t("objet.supprimer")}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onSupprimer();
        }}
      >
        ×
      </button>
    </div>
  );
}

/**
 * L'angle qu'un geste de rotation rend.
 *
 * L'angle du pointeur autour du centre, moins celui d'où il est parti, ajouté
 * à l'angle de départ. **L'écran tourne dans l'autre sens que le moteur**, et
 * c'est la seule raison du signe : la scène garde le sens trigonométrique,
 * l'écran a son y vers le bas.
 *
 * Le pas est le degré, parce qu'un objet posé de travers d'un dixième de degré
 * est un objet posé de travers par accident. ⌥ rend le dixième, comme ⌥ affine
 * déjà le recadrage.
 */
function tourne(
  g: {
    depart: { x: number; y: number };
    centre: { x: number; y: number };
    pose: PoseObjet;
  },
  e: React.PointerEvent,
): number {
  const vers = (x: number, y: number) =>
    (Math.atan2(y - g.centre.y, x - g.centre.x) * 180) / Math.PI;
  const delta = vers(e.clientX, e.clientY) - vers(g.depart.x, g.depart.y);
  const brut = g.pose.angle - delta;
  const pas = e.altKey ? 10 : 1;
  const arrondi = Math.round(brut * pas) / pas;
  // Ramené dans (-180, 180] : un angle qui s'accumule au fil des gestes finit
  // par s'écrire en milliers de degrés dans `album.json`, et ce fichier se
  // relit à la main.
  return ((((arrondi + 180) % 360) + 360) % 360) - 180;
}

/**
 * Redimensionner par un coin, dans le repère propre de la boîte.
 *
 * Le coin opposé reste fixe, et c'est **son** point du monde qui ancre le
 * calcul : on ramène le déplacement dans les axes de la boîte, on y lit la
 * nouvelle largeur et la nouvelle hauteur, puis on replace le centre. Une
 * boîte tournée se retaille ainsi le long de ses propres bords, ce qui est le
 * seul comportement qu'une main attend d'une poignée de coin.
 */
export function tailler(
  pose: PoseObjet,
  coin: number,
  dx: number,
  dy: number,
): PoseObjet {
  const { rect, angle } = pose;
  const c = centre(rect);
  const sommets = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.w, y: rect.y },
    { x: rect.x + rect.w, y: rect.y + rect.h },
    { x: rect.x, y: rect.y + rect.h },
  ];
  const fixeLocal = sommets[COINS[coin].fixe];
  const fixe = angle === 0 ? fixeLocal : tourner(fixeLocal, c, angle);
  const tire = sommets[coin];
  const monde = angle === 0 ? tire : tourner(tire, c, angle);
  const p = { x: monde.x + dx, y: monde.y + dy };

  // Le vecteur du coin fixe vers le pointeur, ramené dans les axes de la
  // boîte : une rotation inverse, et on est revenu à un rectangle droit.
  const v = { x: p.x - fixe.x, y: p.y - fixe.y };
  const local = angle === 0 ? v : tourner(v, { x: 0, y: 0 }, -angle);
  const w = Math.max(Math.abs(local.x), MIN_MM);
  const h = Math.max(Math.abs(local.y), MIN_MM);

  // Le centre de la nouvelle boîte : à mi-chemin du coin fixe, dans les axes
  // de la boîte, puis remis dans le monde.
  const demi = {
    x: (Math.sign(local.x) || 1) * (w / 2),
    y: (Math.sign(local.y) || 1) * (h / 2),
  };
  const versCentre = angle === 0 ? demi : tourner(demi, { x: 0, y: 0 }, angle);
  const nc = { x: fixe.x + versCentre.x, y: fixe.y + versCentre.y };
  return { angle, rect: { x: nc.x - w / 2, y: nc.y - h / 2, w, h } };
}
