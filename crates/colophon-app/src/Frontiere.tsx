// The error boundary over the whole tree. Without one, anything thrown
// during a render unmounts everything and leaves a white window: no
// sentence, no log, no way back — and the unsaved album dies with it. The
// project knew the defect (the format switch was hardened against exactly
// this) and had no net under the rest.
//
// What it does is small and honest. It says what happened, says what is and
// is not lost (what was saved is on disk; what was not, is not), offers the
// detail for a report, and one button that restarts the screen. It does not
// try to save: an album whose render just threw is not one to write over
// the file that still opens.

import React from "react";
import { t } from "./i18n";

type Etat = { erreur: Error | null; copie: boolean };

export class Frontiere extends React.Component<
  { children: React.ReactNode },
  Etat
> {
  state: Etat = { erreur: null, copie: false };

  static getDerivedStateFromError(erreur: Error): Etat {
    return { erreur, copie: false };
  }

  componentDidCatch(erreur: Error, info: React.ErrorInfo) {
    // The console is the only log this side has; the detail also travels
    // to the clipboard through the button below.
    console.error("frontière :", erreur, info.componentStack);
  }

  detail(): string {
    const e = this.state.erreur;
    if (!e) return "";
    return `${e.name}: ${e.message}\n${e.stack ?? ""}`.trim();
  }

  copier = async () => {
    try {
      await navigator.clipboard.writeText(this.detail());
    } catch {
      const ta = document.createElement("textarea");
      ta.value = this.detail();
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
    this.setState({ copie: true });
  };

  render() {
    if (!this.state.erreur) return this.props.children;
    return (
      <div className="empty frontiere" role="alert">
        <div className="empty-block">
          <p className="kicker">Colophon</p>
          <div className="setup">
            <h1 className="setup-heading">{t("frontiere.titre")}</h1>
            <p className="lede">{t("frontiere.lede")}</p>
            <p className="setup-actions">
              <button
                className="cta"
                autoFocus
                onClick={() => window.location.reload()}
              >
                {t("frontiere.recharger")}
              </button>
              <button className="link" onClick={() => void this.copier()}>
                {this.state.copie ? t("frontiere.copie") : t("frontiere.copier")}
              </button>
            </p>
            <p className="frontiere-note">{t("frontiere.signaler")}</p>
            <details className="fault-detail">
              <summary>{t("erreur.detail")}</summary>
              <pre>{this.detail()}</pre>
            </details>
          </div>
        </div>
      </div>
    );
  }
}
