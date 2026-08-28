// The three adjustments of one photograph: exposure, contrast, black and
// white — plus the way back. Not a panel and not a menu entry: native
// controls in a bar that is already tabbable, shown when a case is chosen
// (and in the cover editor, same component — a cover photo absent from the
// spreads must stay adjustable, or it is a dead end).
//
// The gesture follows the crop editor's rule: while a slider moves, only
// the store's draft moves (every surface showing the photo follows live);
// the release commits one album edit, so ⌘Z undoes one slider run, not
// every wiggle of it.

import { useRef } from "react";
import type { Reglage } from "./album";
import { estIdentite, REGLAGE_BORNE, reglageOuIdentite } from "./reglage";
import { poserBrouillon, reglageDe, useReglages } from "./reglages";
import { t } from "./i18n";

export function ReglageBloc({
  src,
  onCommit,
}: {
  src: string;
  /** One history step: App routes this through `edits.ts::setReglage`. */
  onCommit: (src: string, reglage: Reglage) => void;
}) {
  useReglages();
  const shown = reglageOuIdentite(reglageDe(src));
  // A drag is running: commit at its end, not at every input event.
  const dragging = useRef(false);

  const draft = (patch: Partial<Reglage>) =>
    poserBrouillon(src, { ...shown, ...patch });
  const commit = (patch: Partial<Reglage> = {}) => {
    dragging.current = false;
    onCommit(src, { ...shown, ...patch });
    poserBrouillon(src, null);
  };

  const glissiere = (
    cle: "expo" | "contraste",
    libelle: string,
  ) => (
    <label className="reglage-champ" title={libelle}>
      <span className="reglage-libelle">{libelle}</span>
      <input
        type="range"
        min={-REGLAGE_BORNE}
        max={REGLAGE_BORNE}
        step={0.05}
        value={shown[cle]}
        aria-label={libelle}
        onPointerDown={() => {
          dragging.current = true;
        }}
        onChange={(e) => {
          const v = Number(e.target.value);
          // Keyboard steps have no release to wait for: each one commits.
          if (dragging.current) draft({ [cle]: v });
          else commit({ [cle]: v });
        }}
        onPointerUp={() => {
          if (dragging.current) commit();
        }}
        onPointerCancel={() => {
          dragging.current = false;
          poserBrouillon(src, null);
        }}
      />
    </label>
  );

  return (
    <span className="reglage-bloc">
      {glissiere("expo", t("reglage.exposition"))}
      {glissiere("contraste", t("reglage.contraste"))}
      <label className="reglage-champ reglage-nb" title={t("reglage.nb")}>
        <input
          type="checkbox"
          checked={shown.nb}
          aria-label={t("reglage.nb")}
          onChange={(e) => commit({ nb: e.target.checked })}
        />
        <span className="reglage-libelle">{t("reglage.nb")}</span>
      </label>
      {!estIdentite(shown) && (
        <button
          className="link reglage-rendre"
          onClick={() => commit({ expo: 0, contraste: 0, nb: false })}
        >
          {t("reglage.rendre")}
        </button>
      )}
    </span>
  );
}
