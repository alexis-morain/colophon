// La fiche d'une photo, sans le DOM : ce que `FichePhoto.tsx` écrit dans
// chaque case, formaté dans la langue de l'écran. Le moteur rend des
// nombres et des chaînes neutres (`1/250`, des octets, des degrés) ; la
// virgule, l'unité et l'arrondi se décident ici, et se testent sans fenêtre.

import { effectivePpi } from "./album";
import { langue, t } from "./i18n";

/** Un nombre avec au plus `decimales` décimales, les zéros de queue
 *  coupés, la virgule en français. */
export function formatNombre(n: number, decimales: number): string {
  const fixe = n.toFixed(decimales);
  const coupe = decimales > 0 ? fixe.replace(/\.?0+$/, "") : fixe;
  return langue() === "fr" ? coupe.replace(".", ",") : coupe;
}

/** Le temps de pose tel que le moteur l'écrit (`1/250`, `2.5`, `30`), avec
 *  l'unité ; la fraction reste une fraction, le décimal prend la virgule. */
export function formatTempsDePose(s: string): string {
  const valeur = s.includes("/") ? s : formatNombre(Number(s), 1);
  return t("fiche.secondes", { t: valeur });
}

/** Degrés décimaux, quatre décimales, signés : ce qu'on colle dans
 *  n'importe quelle carte. */
export function formatCoordonnees(lat: number, lon: number): string {
  const fixe = (x: number) => {
    const f = x.toFixed(4);
    return langue() === "fr" ? f.replace(".", ",") : f;
  };
  return `${fixe(lat)}, ${fixe(lon)}`;
}

/** Le poids d'un fichier en unités décimales, comme le Finder. */
export function formatOctets(n: number): string {
  if (n < 1000) return `${n} ${t("fiche.unite.o")}`;
  if (n < 1e6) return `${formatNombre(n / 1e3, 0)} ${t("fiche.unite.ko")}`;
  if (n < 1e9) return `${formatNombre(n / 1e6, 1)} ${t("fiche.unite.mo")}`;
  return `${(n / 1e9).toFixed(2).replace(".", langue() === "fr" ? "," : ".")} ${t("fiche.unite.go")}`;
}

/** La résolution de la photo dans sa case, par la règle du prévol
 *  (`effectivePpi`), sur les pixels que la fiche connaît : l'original, pas
 *  la vignette. `null` sans case, ou sans pixels. */
export function ppiDansSaCase(
  fiche: { largeur: number; hauteur: number },
  rect: { w: number; h: number } | null,
  zoom: number,
): number | null {
  if (!rect || fiche.largeur <= 0 || fiche.hauteur <= 0) return null;
  const p = effectivePpi(rect, fiche.largeur, fiche.hauteur, zoom);
  return Number.isFinite(p) ? Math.round(p) : null;
}
