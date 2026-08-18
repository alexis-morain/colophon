// The report panel (Aide → Signaler) : the whole report on screen before
// anything moves, one button to the pre-filled GitHub issue, one to the
// clipboard for the offline or accountless case. Nothing is uploaded from
// here, ever: attaching a picture of a spread stays a deliberate manual act
// on the GitHub page, never a default of ours.

import { useEffect, useMemo, useState } from "react";
import { Album } from "./album";
import { openReportUrl, reportData, ReportData } from "./bridge";
import { t } from "./i18n";
import { fitReport, signalTitle, SignalKind } from "./signaler";

export function SignalerView({
  kind,
  album,
  index,
  selected,
  onClose,
}: {
  kind: SignalKind;
  album: Album | null;
  /** Current spread in the book, -1 on the cover. */
  index: number;
  /** Selected cell of that spread, when the book view has one. */
  selected: number | null;
  onClose: () => void;
}) {
  const [data, setData] = useState<ReportData | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [attach, setAttach] = useState(false);
  const [copie, setCopie] = useState(false);
  const [ouverte, setOuverte] = useState(false);

  useEffect(() => {
    reportData()
      .then(setData)
      .catch((e) => setErr(String(e)));
  }, []);

  // Displayed and sent are one string: what the panel shows is exactly what
  // the button carries, log extract trimmed to fit the URL.
  const fitted = useMemo(
    () => (data ? fitReport(data, album, index, selected, kind, attach) : null),
    [data, album, index, selected, kind, attach],
  );

  const copier = async () => {
    if (!fitted) return;
    try {
      await navigator.clipboard.writeText(fitted.report);
    } catch {
      // WKWebView sans l'API presse-papier : la sélection fait le travail.
      const ta = document.createElement("textarea");
      ta.value = fitted.report;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
    setCopie(true);
  };

  const ouvrir = async () => {
    if (!fitted) return;
    try {
      await openReportUrl(fitted.url);
      setOuverte(true);
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <div className="raccourcis" onClick={onClose}>
      <div
        className="raccourcis-panel signaler-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="raccourcis-head">
          <h2>{signalTitle(kind)}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>
        <p className="signaler-intro">{t("signaler.intro")}</p>
        {err && <p className="signaler-erreur">{err}</p>}
        <pre className="signaler-rapport">
          {fitted?.report ?? t("signaler.construction")}
        </pre>
        <label className="signaler-piece">
          <input
            type="checkbox"
            checked={attach}
            onChange={(e) => setAttach(e.target.checked)}
          />
          {t("signaler.piece")}
        </label>
        <div className="signaler-actions">
          <button className="cta small" disabled={!fitted} onClick={ouvrir}>
            {t("signaler.ouvrir")}
          </button>
          <button className="link" disabled={!fitted} onClick={copier}>
            {copie ? t("signaler.copie") : t("signaler.copier")}
          </button>
        </div>
        <p className="signaler-note">
          {ouverte ? t("signaler.note.ouverte") : t("signaler.note.copie")}
        </p>
      </div>
    </div>
  );
}
