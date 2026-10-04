// Où envoyer ce PDF. The last screen before a file leaves the machine.
//
// Three things at once, in the order a human needs them: what is wrong and
// where (clickable, each defect jumps to its spread), the printer the file is
// for, and the sheet of specifications a printer asks for on the phone.
//
// One printer, Cloudprinter, and its button prepares the folder of the
// order. The PDF without constraints stays one link away; Prodigi and Lulu
// are engine and command-line profiles only (`envoi.ts`).
//
// Nothing here is computed in the browser. The preflight, the spec sheet and
// the spine all come from the engine, per profile: two suppliers disagree on
// every field, and a second copy of their specs in TypeScript would be a
// second thing to keep true.

import { useEffect, useRef, useState } from "react";
import { phraseAbsents } from "./BasculeView";
import { CommandeView } from "./CommandeView";
import { Album } from "./album";
import {
  Defaut,
  Preparation,
  Printer,
  PrevolReport,
  montrerDossierPrepare,
  openReportUrl,
  preflight,
} from "./bridge";
import { PROFIL_ENVOI, PROFIL_LIBRE, actionEnvoi, imprimeurAffiche, phrasePreparation } from "./envoi";
import { t } from "./i18n";
import { VERDICT_URL } from "./signaler";

const mm = (v: number) => v.toFixed(1).replace(".", ",");

export function EnvoiView({
  album,
  printers,
  profil,
  onProfil,
  onJump,
  onExport,
  onPrepare,
  preparation,
  exporting,
  exporte,
  dirty,
  colophonPossible,
  colophonActif,
  onColophon,
  gardeActif,
  onGarde,
  policeManquante,
  absents,
}: {
  album: Album;

  /** Loaded once by the window, shared with the cover editor. */
  printers: Printer[] | null;
  profil: string;
  onProfil: (id: string) => void;
  /** Show spread `n` (1-based) in the book view. */
  onJump: (planche: number) => void;
  /** Le PDF sans contrainte : un fichier, où la boîte le pose. */
  onExport: () => void;
  /** Chez qui relie deux fichiers : le dossier de la commande. */
  onPrepare: () => void;
  /** Ce que la dernière préparation a écrit et ce que le contrôle relu en
   *  dit. Null tant que rien n'a été préparé pour cet album. */
  preparation: Preparation | null;
  exporting: boolean;
  /** A print PDF was written for this album: the verdict form is offered. */
  exporte: boolean;
  /** Unsaved edits: the preflight reads the disk, so it would lie. */
  dirty: boolean;
  /** The album carries the facts the page is made of. False on albums
   *  composed before the page existed: nothing to offer, so nothing shows. */
  colophonPossible: boolean;
  colophonActif: boolean;
  onColophon: (on: boolean) => void;
  /** The half-title travels with the colophon: both need the facts the
   *  composition measured, so `colophonPossible` answers for the two. */
  gardeActif: boolean;
  onGarde: (on: boolean) => void;
  /** La police que l'album nomme n'est plus dans son dossier. L'export ne
   *  échoue pas — il sort dans celle du moteur —, et c'est bien pour ça que
   *  ça se dit ici : c'est le dernier écran avant l'imprimeur, et un livre
   *  composé dans une police que personne n'a choisie se découvre au colis. */
  policeManquante: boolean;
  /** The album's characters its face cannot draw: printed as « ? ». */
  absents: string[];
}) {

  const [report, setReport] = useState<PrevolReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  // Le dossier préparé avant cette ouverture d'Envoi : « Commander » ne
  // s'offre que sur un dossier préparé depuis (décision 3 de K-s3). L'écran
  // se démonte quand on le quitte, donc le montage est l'ouverture.
  const preparationALOuverture = useRef(preparation);
  const preparationNeuve =
    preparation && preparation !== preparationALOuverture.current && profil !== PROFIL_LIBRE
      ? preparation
      : null;

  // Re-run on every profile change, on every arrival, and the moment the
  // album stops being dirty: the preflight reads the disk, so a save is what
  // makes a correction real to it, and the verdict has to follow within the
  // second rather than waiting for the user to leave and come back.
  useEffect(() => {
    let alive = true;
    setRunning(true);
    setError(null);
    preflight(profil).then(
      (r) => alive && (setReport(r), setRunning(false)),
      (e) => alive && (setError(String(e)), setReport(null), setRunning(false)),
    );
    return () => {
      alive = false;
    };
  }, [profil, album, dirty]);

  const chosen = imprimeurAffiche(printers, profil);
  const libre = profil === PROFIL_LIBRE;
  const action = chosen ? actionEnvoi(chosen) : null;
  const bloquants = report?.defauts.filter((d) => d.bloquant) ?? [];
  const avertissements = report?.defauts.filter((d) => !d.bloquant) ?? [];

  return (
    <div className="envoi">
      <section className="envoi-verdict">
        {libre && (
          <p className="envoi-libre">
            {t("envoi.libre.tete")}{" "}
            <button className="link" onClick={() => onProfil(PROFIL_ENVOI)}>
              {t("envoi.libre.retour")}
            </button>
          </p>
        )}
        {policeManquante && (
          <p className="envoi-dirty">{t("police.manquante")}</p>
        )}
        {absents.length > 0 && (
          <p className="envoi-dirty">{phraseAbsents(absents)}</p>
        )}
        {dirty && (
          <p className="envoi-dirty">{t("envoi.dirty")}</p>

        )}
        {error ? (
          <>
            <h2 className="envoi-ko">{t("envoi.prevol.echec")}</h2>
            <details className="fault-detail">
              <summary>{t("erreur.detail")}</summary>
              <pre>{error}</pre>
            </details>
          </>
        ) : running || !report ? (
          <h2 className="envoi-wait">{t("envoi.controle")}</h2>
        ) : report.ok ? (
          <>
            <h2 className="envoi-ok">
              {t("envoi.ok", { imprimeur: report.fiche.imprimeur })}
            </h2>
            <p className="envoi-sub">
              {report.fiche.fichiers === "deux"
                ? t("envoi.ok.deux", {
                    planches: report.fiche.planches,
                    pages: report.fiche.pages_interieur,
                  })
                : t("envoi.ok.un", {
                    planches: report.fiche.planches,
                    pages: report.fiche.pages_interieur,
                    fichier: report.fiche.pages_fichier ?? 0,
                  })}
            </p>
          </>
        ) : (
          <>
            <h2 className="envoi-ko">
              {bloquants.length === 1
                ? t("envoi.ko.un")
                : t("envoi.ko", { n: bloquants.length })}
            </h2>
            <p className="envoi-sub">{t("envoi.ko.sub")}</p>
          </>
        )}
      </section>

      {(bloquants.length > 0 || avertissements.length > 0) && (
        <section className="envoi-defauts">
          {bloquants.map((d, i) => (
            <DefautLigne key={`b${i}`} d={d} onJump={onJump} />
          ))}
          {avertissements.map((d, i) => (
            <DefautLigne key={`a${i}`} d={d} onJump={onJump} />
          ))}
        </section>
      )}

      {chosen && (
        <section className="envoi-imprimeurs">
          <h3>{t("envoi.imprimeur")}</h3>
          <div className="envoi-imprimeur active">
            <span className="envoi-imprimeur-nom">{chosen.nom}</span>
            <span className="envoi-imprimeur-quoi">
              {chosen.pdf_x === "x4" ? "PDF/X-4" : t("envoi.pdf.simple")} ·{" "}
              {chosen.espace === "rgb" ? t("envoi.rvb") : t("envoi.cmjn")} ·{" "}
              {chosen.fichiers === "deux"
                ? t("envoi.deux.fichiers")
                : t("envoi.un.fichier")}{" "}
              · {chosen.dos.mode === "calcule" ? t("envoi.dos.fournir") : t("envoi.dos.non")}
            </span>
            {chosen.certitude === "provisoire" && (
              <span className="envoi-provisoire" title={chosen.reserves.join(" · ")}>
                {t("envoi.provisoire")}
              </span>
            )}
          </div>
        </section>
      )}

      {report && (
        <section className="envoi-fiche">
          <h3>{t("envoi.fiche.titre")}</h3>
          <dl>
            <Ligne k={t("envoi.fiche.format")}>
              {mm(report.fiche.format_page_mm[0])} × {mm(report.fiche.format_page_mm[1])} mm
            </Ligne>
            <Ligne k={t("envoi.fiche.interieur")}>
              {t("envoi.fiche.interieur.v", {
                planches: report.fiche.planches,
                pages: report.fiche.pages_interieur,
              })}
            </Ligne>
            <Ligne k={t("envoi.fiche.fond")}>
              {t("envoi.fiche.fond.v", {
                haut: mm(report.fiche.fond_perdu_mm.haut),
                bas: mm(report.fiche.fond_perdu_mm.bas),
                ext: mm(report.fiche.fond_perdu_mm.exterieur),
                dos: mm(report.fiche.fond_perdu_mm.dos),
              })}
            </Ligne>
            <Ligne k={t("envoi.fiche.zone")}>{t("envoi.fiche.zone.v", { mm: mm(report.fiche.zone_sure_mm) })}</Ligne>
            <Ligne k={t("envoi.fiche.espace")}>
              {report.fiche.espace === "rgb" ? t("envoi.rvb") : t("envoi.fiche.espace.cmjn")} ·{" "}
              {report.fiche.output_intent}
            </Ligne>
            <Ligne k={t("envoi.fiche.conformite")}>
              {report.fiche.conformite === "x4"
                ? t("envoi.fiche.conformite.x4")
                : t("envoi.fiche.conformite.aucune")}
            </Ligne>
            <Ligne k={t("envoi.fiche.livraison")}>
              {report.fiche.fichiers === "deux"
                ? t("envoi.fiche.livraison.deux")
                : t("envoi.fiche.livraison.un", {
                    n: report.fiche.pages_fichier ?? 0,
                  })}
            </Ligne>
            {report.fiche.feuille_couverture_mm !== undefined && (
              <Ligne k={t("envoi.fiche.feuille")}>
                {mm(report.fiche.feuille_couverture_mm[0])} ×{" "}
                {mm(report.fiche.feuille_couverture_mm[1])} mm
              </Ligne>
            )}
            {report.fiche.dos_mm !== undefined && (
              <Ligne k={t("envoi.fiche.dos")}>
                {t("envoi.fiche.dos.v", {
                  mm: mm(report.fiche.dos_mm),
                  pages: report.fiche.pages_interieur,
                  g: report.fiche.grammage_g_m2 ?? 0,
                })}
              </Ligne>
            )}
            <Ligne k={t("envoi.fiche.resolution")}>{t("envoi.fiche.resolution.v", { dpi: report.fiche.resolution_cible_dpi })}</Ligne>
          </dl>

          {(report.reserves?.length ?? 0) > 0 && (
            <div className="envoi-reserves">
              <h4>{t("envoi.reserves")}</h4>
              <ul>
                {report.reserves!.map((r, i) => (
                  <li key={i}>{r}</li>
                ))}
              </ul>
            </div>
          )}
          {(report.notes?.length ?? 0) > 0 && (
            <div className="envoi-notes">
              {report.notes!.map((n, i) => (
                <p key={i}>{n}</p>
              ))}
            </div>
          )}
        </section>
      )}

      {colophonPossible && (
        <section className="envoi-colophon">
          <label>
            <input
              type="checkbox"
              checked={gardeActif}
              onChange={(e) => onGarde(e.target.checked)}
            />
            {t("envoi.garde.label")}
          </label>
          <p>{t("envoi.garde.note")}</p>
          <label>
            <input
              type="checkbox"
              checked={colophonActif}
              onChange={(e) => onColophon(e.target.checked)}
            />
            {t("envoi.colophon.label")}
          </label>
          <p>{t("envoi.colophon.note")}</p>
        </section>
      )}

      <section className="envoi-actions">
        <button
          className="envoi-exporter"
          onClick={action?.prepare ? onPrepare : onExport}
          disabled={exporting || !report?.ok || !action}
          title={report?.ok ? action?.titre : t("envoi.exporter.bloque")}
        >
          {exporting ? t("envoi.exporter.rendu") : action?.texte ?? t("envoi.exporter")}
        </button>
        {preparation && action?.prepare && (
          <div className="envoi-prepare">
            <p className={preparation.rapport.ok ? "envoi-prepare-ok" : "envoi-prepare-ko"}>
              {phrasePreparation(preparation)}
            </p>
            {preparation.rapport.defauts.some((d) => d.bloquant) && (
              <div className="envoi-defauts">
                {preparation.rapport.defauts
                  .filter((d) => d.bloquant)
                  .map((d, i) => (
                    <DefautLigne key={i} d={d} onJump={onJump} />
                  ))}
              </div>
            )}
            <button className="link" onClick={() => void montrerDossierPrepare()}>
              {t("envoi.prepare.montrer")}
            </button>
          </div>
        )}
        <CommandeView preparationNeuve={preparationNeuve} />
        {!libre && (
          <p className="envoi-autre">
            <button className="link" onClick={() => onProfil(PROFIL_LIBRE)}>
              {t("envoi.autre")}
            </button>
          </p>
        )}
      </section>

      {exporte && (
        <section className="envoi-verdict-appel">
          <h3>{t("envoi.verdict.titre")}</h3>
          <p>{t("envoi.verdict.texte")}</p>
          <button className="link" onClick={() => void openReportUrl(VERDICT_URL)}>
            {t("envoi.verdict.bouton")}
          </button>
        </section>
      )}
    </div>
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

/** One defect, clickable when it belongs to a spread. The cause is what the
 *  user reads; the remedy is the gesture that fixes it. */
function DefautLigne({ d, onJump }: { d: Defaut; onJump: (n: number) => void }) {
  const body = (
    <>
      <span className="envoi-defaut-ou">
        {d.planche
          ? t("envoi.defaut.planche", { n: d.planche })
          : t("envoi.defaut.album")}
      </span>
      <span className="envoi-defaut-cause">{d.cause}</span>
      <span className="envoi-defaut-remede">{d.remede}</span>
    </>
  );
  const cls = "envoi-defaut" + (d.bloquant ? " bloquant" : "");
  return d.planche ? (
    <button className={cls} onClick={() => onJump(d.planche!)}>
      {body}
    </button>
  ) : (
    <div className={cls}>{body}</div>
  );
}
