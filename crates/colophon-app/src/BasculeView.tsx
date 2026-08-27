// Changer le format d'un album déjà composé. Le même album : mêmes planches,
// même ordre, mêmes photos, mêmes recadrages. Seul le gabarit d'une planche
// dont les photos trahiraient leurs nouvelles cellules est replié.
//
// Rien n'est écrit ici. Le format choisi rend un aperçu, l'aperçu se lit, et
// l'appliquer passe par l'historique d'édition : ⌘Z l'annule comme n'importe
// quelle retouche, ⌘S la grave. C'est la raison pour laquelle le moteur rend
// un album au lieu d'en enregistrer un.

import { BasculeBilan, FormatPreset } from "./bridge";
import { t } from "./i18n";

export function BasculeView({
  formats,
  courant,
  choisi,
  apercu,
  enCours,
  onChoisir,
  onAppliquer,
  onClose,
}: {
  formats: FormatPreset[];
  courant: { w: number; h: number };
  choisi: string | null;
  apercu: BasculeBilan | null;
  enCours: boolean;
  onChoisir: (f: FormatPreset) => void;
  onAppliquer: () => void;
  onClose: () => void;
}) {
  const memeFormat = (f: FormatPreset) => f.w === courant.w && f.h === courant.h;

  return (
    <div className="bascule" onClick={onClose}>
      <div className="bascule-panel" onClick={(e) => e.stopPropagation()}>
        <header className="bascule-head">
          <h2>{t("bascule.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>

        <p className="bascule-intro">{t("bascule.intro")}</p>

        <ul className="bascule-formats">
          {formats.map((f) => (
            <li key={f.name}>
              <button
                className={"bascule-format" + (choisi === f.name ? " choisi" : "")}
                onClick={() => onChoisir(f)}
                disabled={memeFormat(f) || enCours}
                aria-pressed={choisi === f.name}
              >
                <span className="bascule-format-nom">{f.about}</span>
                <span className="bascule-format-mm">
                  {f.w} × {f.h} mm
                  {memeFormat(f) ? ` · ${t("bascule.format.courant")}` : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>

        {enCours && <p className="bascule-attente">{t("bascule.calcul")}</p>}

        {apercu && !enCours && (
          <section className="bascule-bilan">
            <h3>{t("bascule.bilan")}</h3>
            <p className="bascule-resume">
              {t("bascule.inchangees", {
                n: apercu.planches_inchangees,
                total: apercu.planches,
              })}
            </p>

            {/* La résolution d'abord : c'est le seul dégât qu'aucune main ne
                rattrape. Un gabarit se rechange d'un clic, une photo à court
                de pixels demande une autre photographie. */}
            {apercu.sous_resolution.length > 0 && (
              <div className="bascule-alerte">
                <p>{t("bascule.sous_resolution", { n: apercu.sous_resolution.length })}</p>
                <ul>
                  {apercu.sous_resolution.slice(0, 6).map((s) => (
                    <li key={`${s.planche}-${s.src}`}>
                      {t("bascule.ppi", {
                        planche: s.planche,
                        src: s.src,
                        avant: Math.round(s.ppi_avant),
                        apres: Math.round(s.ppi_apres),
                      })}
                    </li>
                  ))}
                </ul>
                {apercu.sous_resolution.length > 6 && (
                  <p className="bascule-reste">
                    {t("bascule.et_reste", { n: apercu.sous_resolution.length - 6 })}
                  </p>
                )}
              </div>
            )}

            {apercu.couverture_sous_resolution && (
              <p className="bascule-alerte">
                {t("bascule.couverture", {
                  apres: Math.round(apercu.couverture_sous_resolution.ppi_apres),
                })}
              </p>
            )}

            {apercu.inaptes.length > 0 && (
              <p className="bascule-note">
                {t("bascule.inaptes", {
                  n: apercu.inaptes.length,
                  planches: apercu.inaptes.map((i) => i.planche).join(", "),
                })}
              </p>
            )}

            {apercu.replis.length > 0 && (
              <p className="bascule-note">
                {t("bascule.replis", { n: apercu.replis.length })}
              </p>
            )}

            {apercu.epinglees_touchees.length > 0 && (
              <p className="bascule-note">
                {t("bascule.epinglees", {
                  n: apercu.epinglees_touchees.length,
                  planches: apercu.epinglees_touchees.join(", "),
                })}
              </p>
            )}

            {apercu.tailles_manquantes.length > 0 && (
              <p className="bascule-note">
                {t("bascule.manquantes", { n: apercu.tailles_manquantes.length })}
              </p>
            )}

            <p className="bascule-actions">
              <button className="bascule-appliquer" onClick={onAppliquer}>
                {t("bascule.appliquer")}
              </button>
              <span className="bascule-annulable">{t("bascule.annulable")}</span>
            </p>
          </section>
        )}
      </div>
    </div>
  );
}
