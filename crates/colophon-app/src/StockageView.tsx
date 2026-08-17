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
  if (!secs) return "date inconnue";
  return new Date(secs * 1000).toLocaleDateString("fr-FR", {
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

function ligneDetail(a: AlbumEntry): string {
  if (a.probleme) return a.probleme;
  const bits: string[] = [];
  if (a.spreads !== null) bits.push(`${a.spreads} planches`);
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
      `Supprimer « ${a.title} » ?\n\n` +
        `Cela libère ${poids(a.bytes_total)} et efface la composition : ` +
        `les planches, les recadrages, les légendes et l’aperçu.\n\n` +
        `Vos photos ne sont pas touchées, elles restent dans leur dossier.`,
    );
    if (!ok) return;
    setOccupe(true);
    try {
      const libere = await deleteAlbum(a.id);
      onSupprime(a.id);
      setNote(`« ${a.title} » supprimé, ${poids(libere)} libérés.`);
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
      `Vider les caches de vignettes ?\n\n` +
        `Cela libère ${poids(total)}. Aucun album n’est perdu : les vignettes ` +
        `se reconstruisent à la prochaine ouverture, ce qui prend quelques ` +
        `secondes par album.`,
    );
    if (!ok) return;
    setOccupe(true);
    try {
      const libere = await purgeThumbCaches();
      setNote(`Caches vidés, ${poids(libere)} libérés.`);
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
          <h2>Stockage</h2>
          <button className="link" onClick={onClose}>
            Fermer (Échap)
          </button>
        </header>

        <p className="stockage-total">
          {rapport ? (
            <>
              <strong>{poids(rapport.total)}</strong> sur ce disque, {albums.length}{" "}
              {albums.length > 1 ? "albums" : "album"}. Les photos d’origine ne
              sont pas comptées ici : Colophon ne les copie jamais.
            </>
          ) : (
            "Mesure du dossier de données…"
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
                  {a.id === ouvertId && <em className="stockage-ouvert">ouvert</em>}
                </span>
                <span className="stockage-detail">{ligneDetail(a)}</span>
                <span className="stockage-repartition">
                  vignettes {poids(a.bytes_thumbs)}, aperçu {poids(a.bytes_pdf)}
                </span>
              </div>
              <span className="stockage-poids">{poids(a.bytes_total)}</span>
              <button
                className="link danger"
                disabled={occupe}
                onClick={() => supprimer(a)}
              >
                Supprimer
              </button>
            </li>
          ))}
          {rapport && albums.length === 0 && (
            <li className="stockage-vide">Aucun album composé sur cette machine.</li>
          )}
        </ul>

        <div className="stockage-actions">
          <button
            className="cta small"
            disabled={occupe || caches === 0}
            onClick={purger}
          >
            Vider les caches de vignettes ({poids(caches)})
          </button>
          <button className="link" onClick={() => revealDataDir().catch(() => {})}>
            Ouvrir le dossier
          </button>
        </div>
        <p className="signaler-note">
          Un album supprimé ne se récupère pas. Le dossier de photos, lui, reste
          intact : rien ici n’écrit ou n’efface hors du dossier de Colophon.
        </p>
      </div>
    </div>
  );
}
