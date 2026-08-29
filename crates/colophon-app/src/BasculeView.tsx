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

import { useMemo } from "react";
import { Police } from "./album";
import { BasculeBilan, FormatPreset, PoliceEtat, PoliceOfferte } from "./bridge";
import { t } from "./i18n";
import { nomLisible, parFamille, refusLibelle } from "./police";

/** Au-delà, la liste devient un mur : le filtre est ce qui la rend
 *  praticable, et le nombre qui manque se dit au lieu de disparaître. */
const FAMILLES_MONTREES = 40;

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
  filtre: string;
  onFiltre: (v: string) => void;
  onPolice: (p: PoliceOfferte) => void;
  onRendrePolice: () => void;
  onClose: () => void;
}) {
  const memeFormat = (f: FormatPreset) => f.w === courant.w && f.h === courant.h;

  // Le filtre porte sur ce que l'écran montre — famille, nom élagué — et
  // pas sur le nom PostScript, que personne ne tape.
  const familles = useMemo(() => {
    const q = filtre.trim().toLocaleLowerCase();
    const vues = q
      ? polices.filter((p) =>
          `${p.famille} ${p.nom}`.toLocaleLowerCase().includes(q),
        )
      : polices;
    return parFamille(vues);
  }, [polices, filtre]);
  const montrees = familles.slice(0, FAMILLES_MONTREES);
  const cachees = familles.length - montrees.length;


  return (
    <div className="bascule" onClick={onClose}>
      <div className="bascule-panel" onClick={(e) => e.stopPropagation()}>
        <header className="bascule-head">
          <h2>{t("bascule.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>

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

        {polices.length > 0 && (
          <>
            <label className="police-filtre">
              <span className="police-filtre-label">{t("police.filtre")}</span>
              <input
                type="search"
                value={filtre}
                placeholder={t("police.filtre.exemple")}
                onChange={(e) => onFiltre(e.target.value)}
              />
            </label>

            <ul className="police-familles">
              {/* La face du moteur en tête, toujours re-sélectionnable :
                  revenir en arrière ne doit jamais demander de retrouver
                  laquelle c'était. */}
              <li className="police-famille">
                <button
                  className={"police-face" + (policeAlbum ? "" : " choisie")}
                  onClick={onRendrePolice}
                  aria-pressed={!policeAlbum}
                >
                  <span className="police-face-nom">{t("police.projet")}</span>
                  <span className="police-face-note">{t("police.projet.note")}</span>
                </button>
              </li>
              {montrees.map(({ famille, faces }) => (
                <li key={famille} className="police-famille">
                  <h4 className="police-famille-nom">{famille}</h4>
                  <ul>
                    {faces.map((p) => {
                      const active = policeAlbum?.postscript === p.postscript;
                      return (
                        <li key={p.rang}>
                          {/* `aria-disabled` et non `disabled` : un bouton
                              désactivé sort de l'ordre de tabulation, et la
                              raison du refus deviendrait invisible pour qui
                              parcourt la liste au clavier — or c'est
                              justement elle qu'on a tenu à afficher. */}
                          <button
                            className={
                              "police-face" +
                              (p.refus ? " refusee" : "") +
                              (active ? " choisie" : "")
                            }
                            aria-disabled={!!p.refus}
                            aria-pressed={active}
                            onClick={() => !p.refus && onPolice(p)}
                          >

                            <span className="police-face-nom">{nomLisible(p)}</span>
                            {/* Une face refusée s'affiche, grisée, avec sa
                                raison : la cacher enverrait quelqu'un
                                chercher une police qui est bien là. */}
                            {p.refus && (
                              <span className="police-face-refus">
                                {refusLibelle(p.refus)}
                              </span>
                            )}
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                </li>
              ))}
            </ul>
            {cachees > 0 && (
              <p className="bascule-reste">{t("police.reste", { n: cachees })}</p>
            )}
            {familles.length === 0 && (
              <p className="bascule-reste">{t("police.aucune")}</p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
