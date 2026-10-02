// Ce dont le livre est fait : son format, et sa police.
//
// Les deux propriétés qui changent tout sans rien recomposer, et le panneau
// est le même pour cette raison — pas de sixième panneau, la règle est
// écrite dans `App.tsx`. Changer de format replie les gabarits qu'une photo
// trahirait ; changer de police ne bouge pas une planche, pas une photo, pas
// un recadrage : seules les chasses changent, donc les coupures de ligne, et
// au rendu seulement.
//
// Rien n'est écrit ici sauf le fichier de la police, qui est le seul octet
// que le moteur doit poser à côté de l'album pour qu'il voyage. Le reste
// passe par l'historique d'édition : ⌘Z l'annule comme n'importe quelle
// retouche, ⌘S la grave. C'est la raison pour laquelle le moteur rend un
// album au lieu d'en enregistrer un.
//
// La liste des polices vit dans `ListePolices.tsx`, partagée avec le popover
// d'un bloc.

import { Police } from "./album";
import { BasculeBilan, FormatPreset, PoliceEtat, PoliceOfferte } from "./bridge";
import { Dialogue } from "./Dialogue";
import { t } from "./i18n";
import { ListePolices } from "./ListePolices";
import { nomLisible } from "./police";

export function BasculeView({
  formats,
  courant,
  choisi,
  apercu,
  enCours,
  onChoisir,
  onAppliquer,
  polices,
  policeAlbum,
  policeInfo,
  absents,
  filtre,
  onFiltre,
  onPolice,
  onRendrePolice,
  onClose,
}: {
  formats: FormatPreset[];
  courant: { w: number; h: number };
  choisi: string | null;
  apercu: BasculeBilan | null;
  enCours: boolean;
  onChoisir: (f: FormatPreset) => void;
  onAppliquer: () => void;
  polices: PoliceOfferte[];
  policeAlbum: Police | null;
  policeInfo: PoliceEtat | null;
  /** The album's characters this face cannot draw, from the engine. */
  absents: string[];
  filtre: string;
  onFiltre: (v: string) => void;
  onPolice: (p: PoliceOfferte) => void;
  onRendrePolice: () => void;
  onClose: () => void;
}) {
  const memeFormat = (f: FormatPreset) => f.w === courant.w && f.h === courant.h;

  return (
    <Dialogue titre={t("bascule.titre")} onClose={onClose} className="bascule" panelClassName="bascule-panel">

        <h3 className="bascule-section">{t("bascule.section.format")}</h3>
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

        <h3 className="bascule-section">{t("police.section")}</h3>
        <p className="bascule-intro">{t("police.intro")}</p>

        {/* Ce dans quoi le livre sort, et ce qu'il pèse. Le poids est dit
            plutôt que subi : une face du système sortie de sa collection peut
            faire des mégaoctets, et l'album les porte. */}
        <p className="police-courante">
          {policeAlbum ? (
            <>
              <strong>{nomLisible(policeAlbum)}</strong>

              {policeInfo && !policeInfo.manquante && (
                <span className="police-poids">
                  {" · "}
                  {t("police.poids", { ko: Math.round(policeInfo.octets / 1024) })}
                </span>
              )}
              <button className="link police-rendre" onClick={onRendrePolice}>
                {t("police.rendre")}
              </button>
            </>
          ) : (
            <strong>{t("police.projet")}</strong>
          )}
        </p>
        {policeInfo?.manquante && (
          <p className="bascule-alerte">{t("police.manquante")}</p>
        )}
        {absents.length > 0 && (
          <p className="bascule-alerte">{phraseAbsents(absents)}</p>
        )}

        {polices.length > 0 && (
          <ListePolices
            polices={polices}
            projet={{
              libelle: t("police.projet"),
              note: t("police.projet.note"),
              choisi: !policeAlbum,
              onChoisir: onRendrePolice,
            }}
            estChoisie={(p) => policeAlbum?.postscript === p.postscript}
            onVoix={onPolice}
            onFace={onPolice}
            filtre={filtre}
            onFiltre={onFiltre}
          />
        )}
    </Dialogue>
  );
}


/** « Cette police ne dessine pas N caractères… », la liste entre guillemets,
 *  coupée à douze : au-delà, le nombre dit tout. */
export function phraseAbsents(absents: readonly string[]): string {
  const liste =
    absents.slice(0, 12).map((c) => `« ${c} »`).join(", ") +
    (absents.length > 12 ? "…" : "");
  return absents.length === 1
    ? t("police.absents.un", { liste })
    : t("police.absents", { n: absents.length, liste });
}
