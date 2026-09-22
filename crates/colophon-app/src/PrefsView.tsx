// Preferences (⌘,): the application language, whether Colophon looks for a
// new version at launch, a note about appearance, and which renderer draws a
// spread. Every one of them is a React render, never a restart; the native
// menu is rebuilt by App the moment the language moves.
//
// The update switch sits second, right under the language, because it is the
// only setting here that decides whether anything leaves the machine. Its
// note names what goes out rather than reassuring: somebody who opens this
// panel to check that claim must find the answer, not a slogan.
//
// The renderer belongs here rather than in a build flag because wave 2.5 has
// to measure the two against each other in an installed bundle, on one
// machine, without recompiling between the two readings.

import { Lang, setLangue, t, useLangue } from "./i18n";
import { Veille, setVeille, useVeille } from "./maj";
import { Rendu, setRendu, useRendu } from "./rendu";

const LANGES: [Lang, string][] = [
  ["fr", "Français"],
  ["en", "English"],
];

export function PrefsView({ onClose }: { onClose: () => void }) {
  const lang = useLangue();
  const dessin = useRendu();
  const regarde = useVeille();
  const VEILLES: [Veille, string][] = [
    ["au-lancement", t("prefs.maj.au.lancement")],
    ["jamais", t("prefs.maj.jamais")],
  ];
  const RENDUS: [Rendu, string][] = [
    ["dom", t("prefs.rendu.dom")],
    ["canvas", t("prefs.rendu.canvas")],
  ];
  return (
    <div className="raccourcis" onClick={onClose}>
      <div
        className="raccourcis-panel prefs-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="raccourcis-head">
          <h2>{t("prefs.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>

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

        <h3>{t("prefs.rendu")}</h3>
        <div className="prefs-langues" role="radiogroup" aria-label={t("prefs.rendu")}>
          {RENDUS.map(([id, nom]) => (
            <button
              key={id}
              role="radio"
              aria-checked={dessin === id}
              className={"prefs-langue" + (dessin === id ? " active" : "")}
              onClick={() => setRendu(id)}
            >
              {nom}
            </button>
          ))}
        </div>
        <p className="signaler-note">{t("prefs.rendu.note")}</p>
      </div>
    </div>
  );
}
