// La saveur de la construction. Une seule base de code fait le DMG et la
// version Mac App Store ; la seconde est compilée avec `VITE_COLOPHON_MAS=1`
// (`scripts/mas.sh`), et c'est ici, et seulement ici, que la fenêtre le lit.
// Lue à chaque appel plutôt qu'en constante, pour qu'un test puisse la poser.

/** Vrai dans la version Mac App Store : bac à sable, aucun updater. */
export function saveurMas(): boolean {
  return import.meta.env.VITE_COLOPHON_MAS === "1";
}
