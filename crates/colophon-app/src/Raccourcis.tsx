// The keyboard cheat-sheet (⌘/): every gesture of the app on one overlay,
// grouped the way the menus are. Reachable from the Aide menu and from the
// chord itself; Échap or a click puts it away. Built at render time so a
// language change redraws it.

import { Cle, t } from "./i18n";

const GROUPES: [Cle, [string | Cle, Cle][]][] = [
  [
    "racc.naviguer",
    [
      ["⌘1 ⌘2 ⌘3 ⌘4", "racc.vues"],
      ["racc.k.espace", "racc.planche.suiv"],
      ["racc.k.debut", "racc.premiere"],
      ["P", "racc.reserve"],
      ["⇧⌘P", "racc.fidele"],
      ["racc.k.entree", "racc.passer.revue"],
    ],
  ],
  [
    "racc.editer",
    [
      ["⌘D", "racc.dupliquer"],
      ["⌘L", "racc.figer"],
      ["racc.k.suppr.planches", "racc.supprimer"],
      ["⇧⌘← ⇧⌘→", "racc.envoyer.photo"],
      ["racc.k.suppr.livre", "racc.retirer.photo"],
      ["Tab", "racc.tab.legende"],
    ],
  ],
  [
    "racc.recadrer",
    [
      ["racc.k.glisser", "racc.deplacer.cadrage"],
      ["racc.k.molette", "racc.zoomer"],
      ["0", "racc.remplissage"],
      ["racc.k.doubleclic", "racc.recentrer"],
    ],
  ],
  [
    "racc.revue",
    [
      ["← →", "racc.parcourir"],
      ["R", "racc.repecher"],
      ["X", "racc.ecart"],
      ["racc.k.echap", "racc.sortir"],
    ],
  ],
  [
    "racc.album",
    [
      ["⌘S", "racc.enregistrer"],
      ["⌘Z · ⇧⌘Z", "racc.annuler"],
      ["⇧⌘E", "racc.exporter"],
      ["⌘O · ⌘N", "racc.ouvrir"],
    ],
  ],
];

/** A left-column label is either a literal key cap (⌘D) or a dictionary key
 *  (the few French words: Échap, molette, glisser). */
function cap(touches: string): string {
  return touches.startsWith("racc.") ? t(touches as Cle) : touches;
}

export function RaccourcisView({ onClose }: { onClose: () => void }) {
  return (
    <div className="raccourcis" onClick={onClose}>
      <div className="raccourcis-panel" onClick={(e) => e.stopPropagation()}>
        <header className="raccourcis-head">
          <h2>{t("racc.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>
        <div className="raccourcis-groupes">
          {GROUPES.map(([titre, lignes]) => (
            <section key={titre} className="raccourcis-groupe">
              <h3>{t(titre)}</h3>
              <dl>
                {lignes.map(([touches, quoi]) => (
                  <div key={touches} className="raccourcis-ligne">
                    <dt>{cap(touches)}</dt>
                    <dd>{t(quoi)}</dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
