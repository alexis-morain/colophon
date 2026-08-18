// The end-of-composition screen: what was read, what was kept, what was set
// aside and why, and which of the proposals to open. It is the moment the
// user decides whether to trust the machine, and until recently all they had
// was a progress bar that vanished.
//
// The creation form asks four questions before showing anything. This screen
// is the other half of the answer: one question, three books. The analysis is
// what costs, so the two alternatives were composed from the same photos for
// the price of some arithmetic, and they wait on disk until the first save.

import { useEffect, useState } from "react";
import { BuildBilan, VarianteResume } from "./bridge";
import { Album, Discard } from "./album";
import { t } from "./i18n";
import { REASON_KEYS, reasonLabel } from "./reasons";
import { cachedThumb, loadThumb } from "./thumbs";

export function BilanView({
  bilan,
  album,
  curation,
  variantes,
  choisie,
  onChoisir,
  onOpen,
  onTri,
}: {
  bilan: BuildBilan;
  album: Album;
  curation: Discard[];
  /** The proposals composed beside this one. Empty outside the shell, and on
   *  a recomposition, which is not a choice of album. */
  variantes: VarianteResume[];
  /** Which one is on screen right now, by id, or null for the one asked for. */
  choisie: string | null;
  onChoisir: (id: string | null) => void;
  onOpen: () => void;
  onTri: () => void;
}) {
  const spreads = album.spreads.length;
  const counts = new Map<string, number>();
  for (const d of curation) {
    counts.set(d.reason, (counts.get(d.reason) ?? 0) + 1);
  }
  const reasons = REASON_KEYS.map((key) => ({
    key,
    label: reasonLabel(key),
    count: counts.get(key) ?? 0,
  })).filter((r) => r.count > 0);
  const setAside = curation.length;
  const pct = Math.round(
    (100 * bilan.photos_kept) / Math.max(1, bilan.photos_scanned),
  );

  // The proposal asked for sits first, described in the same words as the
  // others: three cards that can be compared, not one book and two options.
  const cartes: VarianteResume[] =
    variantes.length > 0
      ? [
          {
            id: "",
            nom: t("bilan.demande.nom"),
            about: t("bilan.demande.about"),
            planches: spreads,
            photos: bilan.photos_kept,
            apercu: apercuDe(album),
          },
          ...variantes,
        ]
      : [];

  return (
    <div className="empty">
      <div className="empty-block">
        <p className="kicker">Colophon</p>
        <div className="setup bilan">
          <h1 className="setup-heading">{t("bilan.titre", { titre: album.title })}</h1>
          <p className="bilan-lead">
            <strong>{bilan.photos_scanned}</strong> {t("bilan.lues")}{" "}
            <strong>{bilan.photos_kept}</strong>{" "}
            {bilan.chapters > 1
              ? t("bilan.gardees", {
                  pct,
                  planches: spreads,
                  chapitres: bilan.chapters,
                })
              : t("bilan.gardees.chapitre.un", { pct, planches: spreads })}
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

          {cartes.length > 1 && (
            <>
              <h2 className="bilan-choix-titre">{t("bilan.choix.titre")}</h2>
              <ul className="bilan-choix">
                {cartes.map((v) => (
                  <li key={v.id || "demandee"}>
                    <button
                      className={
                        "bilan-carte" +
                        ((choisie ?? "") === v.id ? " active" : "")
                      }
                      aria-pressed={(choisie ?? "") === v.id}
                      onClick={() => onChoisir(v.id || null)}
                    >
                      <span className="bilan-carte-apercu">
                        {v.apercu.map((src) => (
                          <Vignette key={src} src={src} />
                        ))}
                      </span>
                      <span className="bilan-carte-nom">{v.nom}</span>
                      <span className="bilan-carte-chiffres">
                        {t("bilan.carte.chiffres", {
                          planches: v.planches,
                          photos: v.photos,
                        })}
                      </span>
                      <span className="bilan-carte-about">{v.about}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}

          <p className="setup-hint">
            {setAside > 0 ? t("bilan.hint.ecartees") : t("bilan.hint.toutes")}
          </p>

          <p className="setup-actions">
            <button className="cta" autoFocus onClick={onOpen}>
              {t("bilan.ouvrir")}
            </button>
            {setAside > 0 && (
              <button className="link" onClick={onTri}>
                {t("bilan.revue", { n: setAside })}
              </button>
            )}
          </p>
          {cartes.length > 1 && (
            <p className="bilan-garde">{t("bilan.garde")}</p>
          )}
        </div>
      </div>
    </div>
  );
}

/** Same rule as the engine's: a quarter, a half and three quarters in, so the
 *  three thumbnails say what the book looks like and not how it opens. */
function apercuDe(album: Album): string[] {
  const avecPhoto = album.spreads.filter((s) => s.slots.length > 0);
  return [1, 2, 3]
    .map((q) => avecPhoto[Math.floor((avecPhoto.length * q) / 4)])
    .filter((s): s is NonNullable<typeof s> => Boolean(s?.slots[0]))
    .map((s) => s.slots[0].src);
}

/** One preview thumbnail. Square-cropped in CSS: three of them side by side
 *  are a rhythm, and a row of mixed aspect ratios is a mess. */
function Vignette({ src }: { src: string }) {
  const [url, setUrl] = useState(() => cachedThumb(src));
  useEffect(() => {
    let alive = true;
    loadThumb(src).then((u) => alive && setUrl(u), () => {});
    return () => {
      alive = false;
    };
  }, [src]);
  return (
    <span className="bilan-vignette">
      {url && <img src={url} alt="" draggable={false} />}
    </span>
  );
}
