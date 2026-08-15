// The end-of-composition report: what was read, what was kept, what was set
// aside and why. Shown once, between the build and the book. It is the moment
// the user decides whether to trust the machine, and until now all they had
// was a progress bar that vanished.

import { BuildBilan } from "./bridge";
import { Album, Discard } from "./album";
import { REASONS } from "./reasons";

export function BilanView({
  bilan,
  album,
  curation,
  onOpen,
  onTri,
}: {
  bilan: BuildBilan;
  album: Album;
  curation: Discard[];
  onOpen: () => void;
  onTri: () => void;
}) {
  const spreads = album.spreads.length;
  const counts = new Map<string, number>();
  for (const d of curation) {
    counts.set(d.reason, (counts.get(d.reason) ?? 0) + 1);
  }
  const reasons = REASONS.map(([key, label]) => ({
    key,
    label,
    count: counts.get(key) ?? 0,
  })).filter((r) => r.count > 0);
  const setAside = curation.length;
  const pct = Math.round(
    (100 * bilan.photos_kept) / Math.max(1, bilan.photos_scanned),
  );

  return (
    <div className="empty">
      <div className="empty-block">
        <p className="kicker">Colophon</p>
        <div className="setup bilan">
          <h1 className="setup-heading">« {album.title} » est composé</h1>
          <p className="bilan-lead">
            <strong>{bilan.photos_scanned}</strong> photos lues,{" "}
            <strong>{bilan.photos_kept}</strong> dans l’album, soit {pct} % du
            dossier&nbsp;: {spreads} planches en {bilan.chapters} chapitre
            {bilan.chapters > 1 ? "s" : ""}.
          </p>

          {setAside > 0 && (
            <ul className="bilan-reasons">
              {reasons.map((r) => (
                <li key={r.key}>
                  <span className="bilan-count">{r.count}</span>
                  <span className="bilan-label">{r.label}</span>
                </li>
              ))}
            </ul>
          )}

          <p className="setup-hint">
            {setAside > 0
              ? "Rien n’est supprimé : chaque photo écartée attend dans la " +
                "vue Tri, avec sa raison, et un double-clic la repêche."
              : "Toutes les photos du dossier sont dans l’album."}
          </p>

          <p className="setup-actions">
            <button className="cta" autoFocus onClick={onOpen}>
              Ouvrir l’album
            </button>
            {setAside > 0 && (
              <button className="link" onClick={onTri}>
                Passer les {setAside} écartées en revue
              </button>
            )}
          </p>
        </div>
      </div>
    </div>
  );
}
