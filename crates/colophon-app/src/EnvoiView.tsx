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
        {dirty && (
          <p className="envoi-dirty">
            Des modifications ne sont pas enregistrées. Le prévol lit le fichier
            sur le disque : enregistrez (⌘S) avant de vous fier au verdict.
          </p>
        )}
        {error ? (
          <h2 className="envoi-ko">{error}</h2>
        ) : running || !report ? (
          <h2 className="envoi-wait">Contrôle du fichier…</h2>
        ) : report.ok ? (
          <>
            <h2 className="envoi-ok">
              Rien ne s’oppose à l’impression chez {report.fiche.imprimeur}.
            </h2>
            <p className="envoi-sub">
              {report.fiche.planches} planches, {report.fiche.pages_interieur}{" "}
              pages, {report.fiche.fichiers === "deux"
                ? "intérieur et couverture en deux fichiers"
                : `un seul fichier de ${report.fiche.pages_fichier} pages, couverture comprise`}
              .
            </p>
          </>
        ) : (
          <>
            <h2 className="envoi-ko">
              {bloquants.length === 1
                ? "Un défaut arrête l’envoi."
                : `${bloquants.length} défauts arrêtent l’envoi.`}
            </h2>
            <p className="envoi-sub">
              Chaque ligne mène à sa planche. Corrigez, revenez, le contrôle se
              refait tout seul.
            </p>
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
        <h3>Qui accepte un PDF comme celui-ci</h3>
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
                  {p.pdf_x === "x4" ? "PDF/X-4" : "PDF simple"} ·{" "}
                  {p.espace === "rgb" ? "RVB" : "CMJN FOGRA39"} ·{" "}
                  {p.fichiers === "deux" ? "deux fichiers" : "un fichier"} ·{" "}
                  {p.dos.mode === "calcule" ? "dos à fournir" : "dos non demandé"}
                </span>
                {p.certitude === "provisoire" && (
                  <span className="envoi-provisoire" title={p.reserves.join(" · ")}>
                    fiche provisoire
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </section>

      {report && (
        <section className="envoi-fiche">
          <h3>La fiche à donner à l’imprimeur</h3>
          <dl>
            <Ligne k="Format d’une page">
              {mm(report.fiche.format_page_mm[0])} × {mm(report.fiche.format_page_mm[1])} mm
            </Ligne>
            <Ligne k="Intérieur">
              {report.fiche.planches} planches, {report.fiche.pages_interieur} pages
            </Ligne>
            <Ligne k="Fond perdu">
              haut {mm(report.fiche.fond_perdu_mm.haut)}, bas{" "}
              {mm(report.fiche.fond_perdu_mm.bas)}, extérieur{" "}
              {mm(report.fiche.fond_perdu_mm.exterieur)}, dos{" "}
              {mm(report.fiche.fond_perdu_mm.dos)} mm
            </Ligne>
            <Ligne k="Zone sûre">{mm(report.fiche.zone_sure_mm)} mm depuis la coupe</Ligne>
            <Ligne k="Espace couleur">
              {report.fiche.espace === "rgb" ? "RVB" : "CMJN"} · {report.fiche.output_intent}
            </Ligne>
            <Ligne k="Conformité">
              {report.fiche.conformite === "x4" ? "PDF/X-4 déclaré" : "aucune demandée"}
            </Ligne>
            <Ligne k="Livraison">
              {report.fiche.fichiers === "deux"
                ? "deux fichiers : l’intérieur et la couverture à plat"
                : `un seul fichier de ${report.fiche.pages_fichier} pages : couverture en première et en dernière page`}
            </Ligne>
            {report.fiche.dos_mm !== undefined && (
              <Ligne k="Dos">
                {mm(report.fiche.dos_mm)} mm pour {report.fiche.pages_interieur} pages à{" "}
                {report.fiche.grammage_g_m2} g/m²
              </Ligne>
            )}
            <Ligne k="Résolution visée">{report.fiche.resolution_cible_dpi} dpi</Ligne>
          </dl>

          {(report.reserves?.length ?? 0) > 0 && (
            <div className="envoi-reserves">
              <h4>Ce que cette fiche attend encore</h4>
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
            Imprimer la page de garde
          </label>
          <p>
            La première page du livre, comme dans un livre imprimé : le titre,
            les dates du voyage, les villes traversées. Rien d’autre, et deux
            pages de plus.
          </p>
          <label>
            <input
              type="checkbox"
              checked={colophonActif}
              onChange={(e) => onColophon(e.target.checked)}
            />
            Imprimer la page de colophon
          </label>
          <p>
            La dernière page du livre, écrite par le logiciel : combien de
            photographies sur combien, quand, où, avec quels appareils. Deux
            pages de plus, et jamais un chemin, une coordonnée ni une légende.
          </p>
        </section>
      )}

      <section className="envoi-actions">
        <button
          className="envoi-exporter"
          onClick={onExport}
          disabled={exporting || !report?.ok}
          title={
            report?.ok
              ? "Rendu à 300 dpi, puis la couverture si l’imprimeur en veut une"
              : "Corrigez d’abord ce qui bloque"
          }
        >
          {exporting ? "Rendu en cours…" : "Enregistrer le PDF d’impression"}
        </button>
        {chosen && !report?.ok && (
          <p className="envoi-porte">
            Un imprimeur sans contrainte accepte souvent ce que {chosen.nom}{" "}
            refuse : essayez « Imprimeur local » ci-dessus pour voir ce qui
            resterait.
          </p>
        )}
      </section>

      {exporte && (
        <section className="envoi-verdict-appel">
          <h3>Votre avis vaut une planche corrigée</h3>
          <p>
            Deux questions, dix secondes : montreriez-vous cet album tel que le
            logiciel l’a composé, et quelles sont ses trois pires planches ?
            Chaque planche citée est examinée une par une.
          </p>
          <button className="link" onClick={() => void openReportUrl(VERDICT_URL)}>
            Répondre sur GitHub (le formulaire pose ces deux questions)
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
        {d.planche ? `Planche ${d.planche}` : "L’album"}
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
