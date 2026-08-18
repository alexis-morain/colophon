// The storage panel (Fichier → Stockage…): what the app has written on this
// disk, and the two ways to take it back. Until this screen existed, four
// albums held 670 Mo with nothing in the interface saying so.
//
// Every sentence here names what is lost. An album row deletes a composition,
// never the photographs it was composed from: the Rust side cannot reach a
// photo folder from this command, and the confirmation says so in words.

import { useCallback, useEffect, useState } from "react";
import {
  AlbumEntry,
  confirmDialog,
  deleteAlbum,
  listAlbums,
  purgeThumbCaches,
  revealDataDir,
  StorageReport,
} from "./bridge";
import { langue, t } from "./i18n";

/** Base 1024, one decimal past a megabyte: the figure has to be comparable
 *  with what the Finder shows beside it. */
export function poids(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  const ko = bytes / 1024;
  if (ko < 1024) return `${Math.round(ko)} ko`;
  const mo = ko / 1024;
  if (mo < 1024) return `${mo < 10 ? mo.toFixed(1) : Math.round(mo)} Mo`;
  return `${(mo / 1024).toFixed(1)} Go`;
}

function dateCourte(secs: number | null): string {
  if (!secs) return t("stockage.date.inconnue");
  return new Date(secs * 1000).toLocaleDateString(
    langue() === "fr" ? "fr-FR" : "en-GB",
    {
      day: "numeric",
      month: "long",
      year: "numeric",
    },
  );
}

function ligneDetail(a: AlbumEntry): string {
  if (a.probleme) return a.probleme;
  const bits: string[] = [];
  if (a.spreads !== null) bits.push(t("stockage.planches", { n: a.spreads }));
  if (a.format) bits.push(`${Math.round(a.format[0])} × ${Math.round(a.format[1])} mm`);
  bits.push(dateCourte(a.modified));
  return bits.join(", ");
}

export function StockageView({
  ouvertId,
  onSupprime,
  onClose,
}: {
  /** Folder name of the album currently open, so its row says why it cannot
   *  be deleted before the user clicks and gets refused. */
  ouvertId: string | null;
  /** An album that just left the disk: the recents list drops it too. */
  onSupprime: (id: string) => void;
  onClose: () => void;
}) {
  const [rapport, setRapport] = useState<StorageReport | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [occupe, setOccupe] = useState(false);

  const relire = useCallback(() => {
    listAlbums()
      .then(setRapport)
      .catch((e) => setErr(String(e)));
  }, []);

  useEffect(relire, [relire]);

  const supprimer = async (a: AlbumEntry) => {
    const ok = await confirmDialog(
      t("stockage.confirme.supprimer", {
        titre: a.title,
        poids: poids(a.bytes_total),
      }),
    );
    if (!ok) return;
    setOccupe(true);
    try {
      const libere = await deleteAlbum(a.id);
      onSupprime(a.id);
      setNote(t("stockage.supprime", { titre: a.title, poids: poids(libere) }));
      setErr(null);
      relire();
    } catch (e) {
      setErr(String(e));
    } finally {
      setOccupe(false);
    }
  };

  const purger = async () => {
    const total = (rapport?.albums ?? []).reduce((n, a) => n + a.bytes_thumbs, 0);
    const ok = await confirmDialog(
      t("stockage.confirme.purger", { poids: poids(total) }),
    );
    if (!ok) return;
    setOccupe(true);
    try {
      const libere = await purgeThumbCaches();
      setNote(t("stockage.purge", { poids: poids(libere) }));
      setErr(null);
      relire();
    } catch (e) {
      setErr(String(e));
    } finally {
      setOccupe(false);
    }
  };

  const albums = rapport?.albums ?? [];
  const caches = albums.reduce((n, a) => n + a.bytes_thumbs, 0);

  return (
    <div className="raccourcis" onClick={onClose}>
      <div
        className="raccourcis-panel stockage-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="raccourcis-head">
          <h2>{t("stockage.titre")}</h2>
          <button className="link" onClick={onClose}>
            {t("commun.fermer")}
          </button>
        </header>

        <p className="stockage-total">
          {rapport ? (
            <>
              <strong>{poids(rapport.total)}</strong>{" "}
              {albums.length > 1
                ? t("stockage.total.suite", { n: albums.length })
                : t("stockage.total.suite.un")}
            </>
          ) : (
            t("stockage.mesure")
          )}
        </p>

        {err && <p className="signaler-erreur">{err}</p>}
        {note && !err && <p className="stockage-note-ok">{note}</p>}

        <ul className="stockage-liste">
          {albums.map((a) => (
            <li key={a.id} className={a.probleme ? "stockage-ligne abime" : "stockage-ligne"}>
              <div className="stockage-ligne-texte">
                <span className="stockage-titre">
                  {a.title}
                  {a.id === ouvertId && (
                    <em className="stockage-ouvert">{t("stockage.ouvert")}</em>
                  )}
                </span>
                <span className="stockage-detail">{ligneDetail(a)}</span>
                <span className="stockage-repartition">
                  {t("stockage.repartition", {
                    vignettes: poids(a.bytes_thumbs),
                    apercu: poids(a.bytes_pdf),
                  })}
                </span>
              </div>
              <span className="stockage-poids">{poids(a.bytes_total)}</span>
              <button
                className="link danger"
                disabled={occupe}
                onClick={() => supprimer(a)}
              >
                {t("stockage.supprimer")}
              </button>
            </li>
          ))}
          {rapport && albums.length === 0 && (
            <li className="stockage-vide">{t("stockage.vide")}</li>
          )}
        </ul>

        <div className="stockage-actions">
          <button
            className="cta small"
            disabled={occupe || caches === 0}
            onClick={purger}
          >
            {t("stockage.purger", { poids: poids(caches) })}
          </button>
          <button className="link" onClick={() => revealDataDir().catch(() => {})}>
            {t("stockage.ouvrir.dossier")}
          </button>
        </div>
        <p className="signaler-note">{t("stockage.note")}</p>
      </div>
    </div>
  );
}
