// Où envoyer ce PDF. The last screen before a file leaves the machine.
//
// Three things at once, in the order a human needs them: what is wrong and
// where (clickable, each defect jumps to its spread), who takes a file like
// this one, and the sheet of specifications a printer asks for on the phone.
//
// Nothing here is computed in the browser. The preflight, the spec sheet and
// the spine all come from the engine, per profile: two suppliers disagree on
// every field, and a second copy of their specs in TypeScript would be a
// second thing to keep true.

import { useEffect, useState } from "react";
import { Album } from "./album";
import { Defaut, Printer, PrevolReport, openReportUrl, preflight } from "./bridge";
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
  exporting,
  exporte,
  dirty,
  colophonPossible,
  colophonActif,
  onColophon,
  gardeActif,
  onGarde,
  policeManquante,
}: {
  album: Album;

  /** Loaded once by the window, shared with the cover editor. */
  printers: Printer[] | null;
  profil: string;
  onProfil: (id: string) => void;
  /** Show spread `n` (1-based) in the book view. */
  onJump: (planche: number) => void;
  onExport: () => void;
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
}) {

  const [report, setReport] = useState<PrevolReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

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

  const chosen = printers?.find((p) => p.id === profil);
  const bloquants = report?.defauts.filter((d) => d.bloquant) ?? [];
  const avertissements = report?.defauts.filter((d) => !d.bloquant) ?? [];

  return (
    <div className="envoi">
      <section className="envoi-verdict">
        {policeManquante && (
          <p className="envoi-dirty">{t("police.manquante")}</p>
        )}
        {dirty && (
          <p className="envoi-dirty">{t("envoi.dirty")}</p>

        )}
        {error ? (
          <h2 className="envoi-ko">{error}</h2>
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

      <section className="envoi-imprimeurs">
        <h3>{t("envoi.imprimeurs")}</h3>
        <ul>
          {(printers ?? []).map((p) => (
            <li key={p.id}>
              <button
                className={"envoi-imprimeur" + (p.id === profil ? " active" : "")}
                onClick={() => onProfil(p.id)}
                aria-pressed={p.id === profil}
              >
                <span className="envoi-imprimeur-nom">{p.nom}</span>
                <span className="envoi-imprimeur-quoi">
                  {p.pdf_x === "x4" ? "PDF/X-4" : t("envoi.pdf.simple")} ·{" "}
                  {p.espace === "rgb" ? t("envoi.rvb") : t("envoi.cmjn")} ·{" "}
                  {p.fichiers === "deux"
                    ? t("envoi.deux.fichiers")
                    : t("envoi.un.fichier")}{" "}
                  · {p.dos.mode === "calcule" ? t("envoi.dos.fournir") : t("envoi.dos.non")}
                </span>
                {p.certitude === "provisoire" && (
                  <span className="envoi-provisoire" title={p.reserves.join(" · ")}>
                    {t("envoi.provisoire")}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </section>

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
          onClick={onExport}
          disabled={exporting || !report?.ok}
          title={
            report?.ok ? t("envoi.exporter.titre") : t("envoi.exporter.bloque")
          }
        >
          {exporting ? t("envoi.exporter.rendu") : t("envoi.exporter")}
        </button>
        {chosen && !report?.ok && (
          <p className="envoi-porte">{t("envoi.porte", { nom: chosen.nom })}</p>
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
