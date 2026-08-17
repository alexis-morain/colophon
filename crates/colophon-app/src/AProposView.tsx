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

/** The three assets the engine embeds, each with the terms it travels under
 *  and the file that carries them in the repo. */
const ACTIFS: [string, string, string][] = [
  [
    "Source Sans 3",
    "SIL Open Font License 1.1, Adobe",
    "La police du livre et de l’interface. C’est elle que le PDF incorpore.",
  ],
  [
    "sRGB2014.icc",
    "International Color Consortium, redistribution sans restriction",
    "Le profil couleur que le PDF embarque comme OutputIntent.",
  ],
  [
    "GeoNames cities5000",
    "Creative Commons Attribution 4.0",
    "Les noms de villes qui titrent les chapitres, depuis le GPS des photos.",
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
          <h2>À propos de Colophon</h2>
          <button className="link" onClick={onClose}>
            Fermer (Échap)
          </button>
        </header>

        <p className="apropos-version">
          Version {data?.version ?? "…"}, sous licence{" "}
          <strong>GNU General Public License v3.0</strong>.
        </p>
        <p className="apropos-quoi">
          Un dossier de photos en entrée, un album composé, tout modifiable, un
          PDF prêt à imprimer. Le code source est public : vous pouvez le lire,
          le modifier et le redistribuer aux mêmes conditions.
        </p>

        <h3>Ce qui voyage à l’intérieur</h3>
        <ul className="apropos-actifs">
          {ACTIFS.map(([nom, licence, quoi]) => (
            <li key={nom}>
              <span className="apropos-actif-nom">{nom}</span>
              <span className="apropos-actif-licence">{licence}</span>
              <span className="apropos-actif-quoi">{quoi}</span>
            </li>
          ))}
        </ul>
        <p className="apropos-attribution">
          Noms de lieux : données GeoNames (https://www.geonames.org), sous
          licence Creative Commons Attribution 4.0.
        </p>

        <p className="apropos-actions">
          <button className="link" onClick={() => setNotices((n) => !n)}>
            {notices
              ? "Masquer les notices des licences tierces"
              : "Notices des licences tierces"}
          </button>
        </p>
        {notices && (
          <pre className="apropos-notices">
            {data?.notices?.trim() ||
              "Les notices n’ont pas été générées pour cette version (scripts/notices.sh)."}
          </pre>
        )}
      </div>
    </div>
  );
}
