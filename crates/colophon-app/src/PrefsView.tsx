// Preferences (⌘,): the application language, whether Colophon looks for a
// new version at launch, and a note about appearance. Every one of them is a React render, never a restart; the native
// menu is rebuilt by App the moment the language moves.
//
// The update switch sits second, right under the language, because it is the
// only setting here that decides whether anything leaves the machine. Its
// note names what goes out rather than reassuring: somebody who opens this
// panel to check that claim must find the answer, not a slogan.
//
// Le choix Éléments / Canvas n'est plus ici : il ne servait à personne qui
// fabrique un album. `rendu.ts` et sa clé `colophon.rendu` restent pour les
// mesures de `scripts/mesure-rendu.md`.

import { Dialogue } from "./Dialogue";
import { Lang, setLangue, t, useLangue } from "./i18n";
import { Veille, setVeille, useVeille } from "./maj";

const LANGES: [Lang, string][] = [
  ["fr", "Français"],
  ["en", "English"],
];

export function PrefsView({ onClose }: { onClose: () => void }) {
  const lang = useLangue();
  const regarde = useVeille();
  const VEILLES: [Veille, string][] = [
    ["au-lancement", t("prefs.maj.au.lancement")],
    ["jamais", t("prefs.maj.jamais")],
  ];
  return (
    <Dialogue titre={t("prefs.titre")} onClose={onClose} panelClassName="prefs-panel">

        <h3>{t("prefs.langue")}</h3>
        <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.langue")}>
          {LANGES.map(([id, nom]) => (
            <button
              key={id}
              role="radio"
              aria-checked={lang === id}
              className={"prefs-langue" + (lang === id ? " active" : "")}
              onClick={() => setLangue(id)}
            >
              {nom}
            </button>
          ))}
        </div>
        <p className="signaler-note">{t("prefs.langue.note")}</p>

        <h3>{t("prefs.maj")}</h3>
        <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.maj")}>
          {VEILLES.map(([id, nom]) => (
            <button
              key={id}
              role="radio"
              aria-checked={regarde === id}
              className={"prefs-langue" + (regarde === id ? " active" : "")}
              onClick={() => setVeille(id)}
            >
              {nom}
            </button>
          ))}
        </div>
        <p className="signaler-note">{t("prefs.maj.note")}</p>

        <h3>{t("prefs.theme")}</h3>
        <p className="signaler-note">{t("prefs.theme.note")}</p>
    </Dialogue>
  );
}
