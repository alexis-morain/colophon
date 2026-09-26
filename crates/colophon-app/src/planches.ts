// Le glisser des planches, en calcul pur : quand un geste devient un
// glisser, et sur quelle planche il tombe. `PlanchesView` mesure les
// rectangles et suit le pointeur ; ici on ne fait que répondre.

/** Sous ce déplacement, un appui reste un clic : la sélection d'avant. */
export const SEUIL_GLISSER_PX = 6;

/** Le rectangle d'une cellule à l'écran. `at` vaut -1 pour la couverture. */
export type RectCellule = {
  at: number;
  left: number;
  top: number;
  right: number;
  bottom: number;
};

/** Le pointeur est allé assez loin pour que l'appui soit un glisser. */
export function seuilFranchi(dx: number, dy: number): boolean {
  return Math.hypot(dx, dy) >= SEUIL_GLISSER_PX;
}

/** La planche sous le pointeur, ou rien. La couverture n'est jamais une
 *  cible : elle a une seule place possible, la première. */
export function cibleSous(rects: RectCellule[], x: number, y: number): number | null {
  for (const r of rects) {
    if (r.at < 0) continue;
    if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return r.at;
  }
  return null;
}
