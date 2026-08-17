// The keyboard cheat-sheet (⌘/): every gesture of the app on one overlay,
// grouped the way the menus are. Reachable from the Aide menu and from the
// chord itself; Échap or a click puts it away.

const GROUPES: [string, [string, string][]][] = [
  [
    "Naviguer",
    [
      ["⌘1 ⌘2 ⌘3 ⌘4", "Livre, Tri, Planches, Envoi"],
      ["← → · espace", "Planche précédente, suivante"],
      ["Début / Fin", "Première, dernière planche"],
      ["P", "Photos en réserve"],
      ["⇧⌘P", "Aperçu fidèle : la page telle que le PDF la contient"],
      ["Entrée (Tri)", "Passer en revue"],
    ],
  ],
  [
    "Éditer la planche",
    [
      ["⌘D", "Dupliquer la planche"],
      ["⌘L", "Figer ou libérer la planche"],
      ["⌫ (Planches)", "Supprimer la planche"],
      ["⇧⌘← ⇧⌘→", "Envoyer la photo sur la planche voisine"],
      ["⌫ (Livre)", "Retirer la photo sélectionnée"],
    ],
  ],
  [
    "Recadrer la photo sélectionnée",
    [
      ["glisser · ⌥ affine", "Déplacer le cadrage"],
      ["molette · + −", "Zoomer, dézoomer"],
      ["0", "Revenir au remplissage exact"],
      ["double-clic", "Recentrer sur le visage détecté"],
    ],
  ],
  [
    "En revue (Tri)",
    [
      ["← →", "Parcourir les écartées"],
      ["R", "Repêcher"],
      ["X", "Écart confirmé, photo suivante"],
      ["Échap", "Sortir de la revue"],
    ],
  ],
  [
    "L’album",
    [
      ["⌘S", "Enregistrer"],
      ["⌘Z · ⇧⌘Z", "Annuler, rétablir"],
      ["⇧⌘E", "Exporter (ouvre Envoi)"],
      ["⌘O · ⌘N", "Ouvrir, nouveau"],
    ],
  ],
];

export function RaccourcisView({ onClose }: { onClose: () => void }) {
  return (
    <div className="raccourcis" onClick={onClose}>
      <div className="raccourcis-panel" onClick={(e) => e.stopPropagation()}>
        <header className="raccourcis-head">
          <h2>Raccourcis clavier</h2>
          <button className="link" onClick={onClose}>
            Fermer (Échap)
          </button>
        </header>
        <div className="raccourcis-groupes">
          {GROUPES.map(([titre, lignes]) => (
            <section key={titre} className="raccourcis-groupe">
              <h3>{titre}</h3>
              <dl>
                {lignes.map(([touches, quoi]) => (
                  <div key={touches} className="raccourcis-ligne">
                    <dt>{touches}</dt>
                    <dd>{quoi}</dd>
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
