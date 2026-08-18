// À propos. What the software is, under which licence, and what travels
// inside it that belongs to somebody else.
//
// Three of those are obligations, not courtesies. The Source Sans 3 face
// ships under the SIL Open Font License, the sRGB profile under the ICC's own
// redistribution terms, and the GeoNames gazetteer under CC BY 4.0, whose one
// condition is that the source be credited. An attribution dropped by
// inattention is a licence violation, so it is written here, in the binary,
// where nobody can lose it.

import { useEffect, useState } from "react";
import { aboutData, AboutData } from "./bridge";
import { Cle, t } from "./i18n";

/** The three assets the engine embeds, each with the terms it travels under.
 *  Names and licences are proper nouns; descriptions translate, and so does
 *  the one licence line that carries a sentence. */
const ACTIFS: [string, () => string, Cle][] = [
  ["Source Sans 3", () => "SIL Open Font License 1.1, Adobe", "apropos.police.quoi"],
  ["sRGB2014.icc", () => t("apropos.icc.licence"), "apropos.icc.quoi"],
  [
    "GeoNames cities5000",
    () => "Creative Commons Attribution 4.0",
    "apropos.geonames.quoi",
  ],
];

export function AProposView({ onClose }: { onClose: () => void }) {
  const [data, setData] = useState<AboutData | null>(null);
  const [notices, setNotices] = useState(false);

  useEffect(() => {
    aboutData().then(setData, () => setData(null));
  }, []);

  return (
    <div className="raccourcis" onClick={onClose}>
      <div
        className="raccourcis-panel apropos-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="raccourcis-head">
          <h2>{t("apropos.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>

        <p className="apropos-version">
          {t("apropos.version", { version: data?.version ?? "…" })}{" "}
          <strong>{t("apropos.licence")}</strong>.
        </p>
        <p className="apropos-quoi">{t("apropos.quoi")}</p>

        <h3>{t("apropos.actifs")}</h3>
        <ul className="apropos-actifs">
          {ACTIFS.map(([nom, licence, quoi]) => (
            <li key={nom}>
              <span className="apropos-actif-nom">{nom}</span>
              <span className="apropos-actif-licence">{licence()}</span>
              <span className="apropos-actif-quoi">{t(quoi)}</span>
            </li>
          ))}
        </ul>
        <p className="apropos-attribution">{t("apropos.attribution")}</p>

        <p className="apropos-actions">
          <button className="link" onClick={() => setNotices((n) => !n)}>
            {notices ? t("apropos.notices.masquer") : t("apropos.notices.voir")}
          </button>
        </p>
        {notices && (
          <pre className="apropos-notices">
            {data?.notices?.trim() || t("apropos.notices.absentes")}
          </pre>
        )}
      </div>
    </div>
  );
}
