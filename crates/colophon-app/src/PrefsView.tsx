// Preferences (⌘,): the application language, and a note about appearance.
// One panel, two facts. The language change is a React render, never a
// restart; the native menu is rebuilt by App the moment the language moves.

import { Lang, setLangue, t, useLangue } from "./i18n";

const LANGES: [Lang, string][] = [
  ["fr", "Français"],
  ["en", "English"],
];

export function PrefsView({ onClose }: { onClose: () => void }) {
  const lang = useLangue();
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

        <h3>{t("prefs.theme")}</h3>
        <p className="signaler-note">{t("prefs.theme.note")}</p>
      </div>
    </div>
  );
}
