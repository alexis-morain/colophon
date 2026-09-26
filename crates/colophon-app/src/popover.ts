// Où s'ouvre un popover ancré à un objet de la planche : la légende sous sa
// photo, les réglages d'un bloc sous le bloc. Position fixe, en pixels de la
// fenêtre, pour qu'il puisse pendre sous la feuille sans être coupé par elle.

/** Un rectangle droit à l'écran. */
export type Boite = { left: number; top: number; right: number; bottom: number };

/** L'écart entre l'objet et son popover, et entre le popover et la fenêtre. */
const ECART = 8;

/** La légende prend la largeur de sa case, entre 240 et 420 px : plus
 *  étroite, le champ ne se lit plus ; plus large, l'œil perd la case. */
export function largeurLegende(largeurCase: number): number {
  return Math.max(240, Math.min(largeurCase, 420));
}

/** Sous l'objet, aligné sur son bord gauche ; au-dessus quand le bas de la
 *  fenêtre est trop proche ; toujours dans la fenêtre. */
export function placerSous(
  ancre: Boite,
  taille: { w: number; h: number },
  fenetre: { w: number; h: number },
): { left: number; top: number } {
  const left = Math.max(ECART, Math.min(ancre.left, fenetre.w - taille.w - ECART));
  const dessous = ancre.bottom + ECART;
  const top =
    dessous + taille.h + ECART > fenetre.h
      ? Math.max(ECART, ancre.top - taille.h - ECART)
      : dessous;
  return { left, top };
}

/** Le rectangle droit qui contient des points : l'emprise d'un objet tourné. */
export function enveloppe(points: { x: number; y: number }[]): Boite {
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return {
    left: Math.min(...xs),
    top: Math.min(...ys),
    right: Math.max(...xs),
    bottom: Math.max(...ys),
  };
}
