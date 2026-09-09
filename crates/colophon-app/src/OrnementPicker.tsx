// Le sélecteur d'ornements : le second bouton de la barre contextuelle, à
// côté de « Bloc de texte ». Pas un sixième panneau — les cinq qui s'excluent
// restent cinq —, et pas d'entrée de menu ni de raccourci : « Bloc de texte »
// n'en a pas non plus.
//
// **Il montre des dessins, pas des noms.** C'est la doctrine de
// `TemplatePicker` : une entrée *est* le dessin, à sa taille de case, et le
// titre du pack ne sert qu'à le nommer pour VoiceOver et au survol. Un
// fleuron se reconnaît d'un coup d'œil et ne se retient pas par son
// identifiant.
//
// Le popover est celui de `TemplatePicker`, au motif près : bouton `.link`,
// ouverture par un état, fermeture au clic dehors et à Échap.

import { useEffect, useRef, useState } from "react";
import { t } from "./i18n";
import {
  attributD,
  attributViewBox,
  Ornement,
  parFamille,
  titre,
  titreDeFamille,
} from "./ornement";

export function OrnementPicker({ onPick }: { onPick: (o: Ornement) => void }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  // Échap et le clic dehors referment avant que quoi que ce soit d'autre ne
  // réagisse : c'est la même capture que le sélecteur de gabarit.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      setOpen(false);
    };
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onDown);
    };
  }, [open]);

  // Le pack est lu à l'ouverture : il ne change pas pendant la vie de la
  // fenêtre, `bridge.ts` le posant une fois au démarrage.
  const groupes = parFamille();

  return (
    <div className="orn" ref={root}>
      <button
        className="link"
        onClick={() => setOpen((o) => !o)}
        title={t("ornement.ajouter.titre")}
        aria-expanded={open}
      >
        {t("ornement.ajouter")}
      </button>
      {open && (
        <div className="orn-panel" role="listbox">
          {/* Un moteur plus vieux que l'app ne rend aucun ornement, et c'est
              un état atteignable : une phrase, jamais une grille vide. */}
          {groupes.length === 0 ? (
            <p className="orn-vide">{t("ornement.vide")}</p>
          ) : (
            groupes.map(([famille, dedans]) => {
              const nom = titreDeFamille(famille);
              return (
                <div
                  className="orn-groupe"
                  key={famille}
                  role="group"
                  aria-label={nom}
                >
                  <h4 className="orn-groupe-nom">{nom}</h4>
                  <div className="orn-groupe-cases">
                    {dedans.map((o) => (
                      <button
                        key={o.id}
                        role="option"
                        aria-selected={false}
                        className="orn-option"
                        title={titre(o)}
                        aria-label={titre(o)}
                        onClick={() => {
                          setOpen(false);
                          onPick(o);
                        }}
                      >
                        <svg
                          className="orn-dessin"
                          aria-hidden="true"
                          viewBox={attributViewBox(o.dessin)}
                          preserveAspectRatio="xMidYMid meet"
                        >
                          {o.dessin.chemins.map((chemin, i) => (
                            <path
                              key={i}
                              d={attributD(chemin)}
                              fill="currentColor"
                              fillRule={chemin.evenodd ? "evenodd" : "nonzero"}
                            />
                          ))}
                        </svg>
                      </button>
                    ))}
                  </div>
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}
