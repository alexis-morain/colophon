// La fiche d'une photo (chantier F) : un `Dialogue`, titre = le nom du
// fichier, deux colonnes comme la fiche d'Envoi. Le moteur lit le fichier
// et le relevé ; l'app n'ajoute que le ppi dans la case, qu'elle seule
// connaît (`effectivePpi`, la règle du prévol), et met les unités.
// Une ligne dont le fichier ne dit rien ne s'affiche pas : une fiche de
// capture d'écran a quatre lignes, pas quinze « ? ».

import { useEffect, useState } from "react";
import { MIN_EFFECTIVE_PPI } from "./album";
import { FichePhoto as Fiche, photoFiche, revelerPhoto } from "./bridge";
import { Dialogue } from "./Dialogue";
import {
  formatCoordonnees,
  formatNombre,
  formatOctets,
  formatTempsDePose,
  ppiDansSaCase,
} from "./fiche";
import { langue, t } from "./i18n";

export function FichePhoto({
  src,
  rect,
  zoom,
  onClose,
}: {
  src: string;
  /** La case de la photo sur la planche affichée, en millimètres, ou rien
   *  quand la fiche s'ouvre sans case. */
  rect: { w: number; h: number } | null;
  zoom: number;
  onClose: () => void;
}) {
  const [fiche, setFiche] = useState<Fiche | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    let vivant = true;
    setFiche(null);
    setErreur(null);
    photoFiche(src).then(
      (f) => vivant && setFiche(f),
      (e) => vivant && setErreur(String(e)),
    );
    return () => {
      vivant = false;
    };
  }, [src]);

  const original = () => {
    void revelerPhoto(src).catch((e) => setErreur(String(e)));
  };

  const ppi = fiche ? ppiDansSaCase(fiche, rect, zoom) : null;

  return (
    <Dialogue titre={fiche?.nom ?? src} onClose={onClose} panelClassName="fiche-panel">
      {erreur && <p className="fiche-erreur">{t("fiche.erreur", { detail: erreur })}</p>}
      {!fiche && !erreur && <p className="fiche-attente">{t("fiche.chargement")}</p>}
      {fiche && (
        <dl className="fiche-photo">
          <Ligne k={t("fiche.dimensions")}>
            {t("fiche.dimensions.v", { l: fiche.largeur, h: fiche.hauteur })}
          </Ligne>
          <Ligne k={t("fiche.poids")}>{formatOctets(fiche.octets)}</Ligne>
          <Ligne k={t("fiche.format")}>{fiche.format}</Ligne>
          <Ligne k={t("fiche.date")}>{dateDe(fiche)}</Ligne>
          {fiche.appareil && <Ligne k={t("fiche.appareil")}>{fiche.appareil}</Ligne>}
          {fiche.objectif && <Ligne k={t("fiche.objectif")}>{fiche.objectif}</Ligne>}
          {fiche.ouverture !== null && (
            <Ligne k={t("fiche.ouverture")}>
              {t("fiche.ouverture.v", { n: formatNombre(fiche.ouverture, 1) })}
            </Ligne>
          )}
          {fiche.temps_de_pose && (
            <Ligne k={t("fiche.temps.de.pose")}>{formatTempsDePose(fiche.temps_de_pose)}</Ligne>
          )}
          {fiche.iso !== null && <Ligne k={t("fiche.iso")}>{fiche.iso}</Ligne>}
          {fiche.focale_mm !== null && (
            <Ligne k={t("fiche.focale")}>
              {t("fiche.mm", { n: formatNombre(fiche.focale_mm, 2) })}
            </Ligne>
          )}
          {fiche.gps && (
            <Ligne k={t("fiche.gps")}>{formatCoordonnees(fiche.gps[0], fiche.gps[1])}</Ligne>
          )}
          {fiche.lieu && (
            <Ligne k={t("fiche.lieu")}>
              {t("fiche.lieu.v", { nom: fiche.lieu.nom, pays: fiche.lieu.pays })}
            </Ligne>
          )}
          {fiche.note !== null && (
            <Ligne k={t("fiche.note")}>
              {fiche.note < 0 ? t("fiche.note.rejetee") : t("fiche.note.etoiles", { n: fiche.note })}
            </Ligne>
          )}
          {fiche.nettete !== null && (
            <Ligne k={t("fiche.nettete")}>{formatNombre(fiche.nettete, 1)}</Ligne>
          )}
          {fiche.exposition !== null && (
            <Ligne k={t("fiche.exposition")}>{formatNombre(fiche.exposition, 2)}</Ligne>
          )}
          {ppi !== null && (
            <Ligne k={t("fiche.ppi")}>
              {t("fiche.ppi.v", { n: ppi })}
              {ppi < MIN_EFFECTIVE_PPI && (
                <>
                  {", "}
                  <span className="fiche-sous">
                    {t("fiche.ppi.sous", { min: MIN_EFFECTIVE_PPI })}
                  </span>
                </>
              )}
            </Ligne>
          )}
        </dl>
      )}
      <div className="fiche-pied">
        <button className="link" onClick={original} disabled={!fiche}>
          {t("ctx.voir.original")}
        </button>
      </div>
    </Dialogue>
  );
}

function Ligne({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <>
      <dt>{k}</dt>
      <dd>{children}</dd>
    </>
  );
}

/** La date de prise de vue dans la langue, à la minute ; une date venue du
 *  fichier plutôt que de l'EXIF le dit, parce qu'elle ne vaut rien. */
function dateDe(fiche: Fiche): string {
  // Un `NaiveDateTime` : ni fuseau ni Z, lu tel quel comme une heure locale.
  const d = new Date(fiche.taken);
  const date = Number.isNaN(d.getTime())
    ? fiche.taken
    : new Intl.DateTimeFormat(langue() === "fr" ? "fr-FR" : "en-GB", {
        dateStyle: "long",
        timeStyle: "short",
      }).format(d);
  return fiche.taken_reliable ? date : t("fiche.date.fichier", { date });
}
