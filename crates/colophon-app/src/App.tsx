import { useCallback, useEffect, useRef, useState } from "react";
import {
  BuildBilan,
  buildAlbum,
  checkUpdate,
  chooseVariante,
  VarianteResume,
  cancelBuild,
  cancelExport,
  confirmDialog,
  exportPdf,
  fetchCuration,
  FormatPreset,
  inTauri,
  chargeGeometrieFormat,
  legendeProposee,
  listDensities,
  DensitePreset,
  listFormats,
  listPrinters,
  onBuildProgress,
  colophonSpread,
  gardeSpread,
  openAlbum as openAlbumAt,
  originSpread,
  pickAlbumFolder,
  pickPhotosFolder,
  Printer,
  gabaritsCompatibles,
  recomposeAlbum,
  basculeAlbum,
  BasculeBilan,
  choisirPolice,
  policeEtat,
  PoliceEtat,
  policeOctets,
  polices_installees,
  PoliceOfferte,

  renderCoverPreview,
  renderPdf,
  saveAlbum,
} from "./bridge";
import {
  Album,
  Discard,
  spreadGeometry,
  OpenedAlbum,
  Reglage,
  Slot,
  slotsFor,
  Spread,
  templateCapacity,
  TITRE_MAX,
} from "./album";
import { adopterGeometrie } from "./geometrie";
import { ReglageBloc } from "./ReglageBloc";
import { filtreDe, poserReglages, useReglages } from "./reglages";
import {
  changeTemplate,
  duplicateSpread,
  insertSpread,
  moveBlocker,
  movePhoto,
  moveSpread,
  placePhoto,
  removePhoto,
  removeSpread,
  renameAlbum,
  setColophon,
  hasColophon,
  setGarde,
  hasGarde,
  rescuePhoto,
  restoreSpread,
  setCover,
  setReglage,
  setSlotCaption,
  setSlotCrop,
  setSpreadCaption,
  setSpreadText,
  spreadOf,
  swapPhotos,
  templateChoices,
  toggleLock,
  triEntries,
  TriEntry,
} from "./edits";
import { BilanView } from "./BilanView";
import { SpreadView } from "./SpreadView";
import { TemplatePicker } from "./TemplatePicker";
import { choixOfferts, faceFor, cleDeForme, formeDe } from "./gabarit";
import { RevueView, TriView } from "./TriView";
import { Drawer } from "./Drawer";
import { PlanchesView, LockGlyph } from "./PlanchesView";
import { CoverView } from "./CoverView";
import { EnvoiView } from "./EnvoiView";
import { Cle, FR, langue, t, useLangue } from "./i18n";
import { jusquAuRendu } from "./mesure";
import { RaccourcisView } from "./Raccourcis";
import { BasculeView } from "./BasculeView";
import { chargerFace } from "./font";
import { nomLisible } from "./police";


import { SignalerView } from "./SignalerView";
import { SignalKind } from "./signaler";
import { Chevron, CoverGlyph } from "./icons";
import { installMenu, MenuActions, RecentAlbum } from "./menu";
import { readRecents, pushRecent, forgetRecent, albumId } from "./recents";
import { StockageView } from "./StockageView";
import { AProposView } from "./AProposView";
import { PrefsView } from "./PrefsView";
import { ApercuFidele } from "./pdfview";
import { forgetPdfs } from "./raster";
import { Tourneur } from "./Feuilletage";
import { cachedThumb, loadThumb, resetThumbs } from "./thumbs";
import "./styles.css";

/** Full album snapshots: a 50-spread album is a few tens of kilobytes. */
type History = { album: Album; past: Album[]; future: Album[] };
const HISTORY_CAP = 50;

/** An error told to the user: a French sentence first, the raw detail behind
 *  a disclosure. Raw `String(e)` never reaches the screen on its own. */
type Fault = { quoi: string; detail: string };

const fault = (quoi: string, e: unknown): Fault => ({
  quoi,
  detail: String(e),
});

/** The banner a Fault renders as: the sentence, the detail folded away. */
function FaultBlock({
  fault,
  onDismiss,
}: {
  fault: Fault;
  onDismiss?: () => void;
}) {
  return (
    <div className="warn fault">
      <p className="fault-quoi">
        {fault.quoi}
        {onDismiss && (
          <button className="link" onClick={onDismiss}>
            Fermer
          </button>
        )}
      </p>
      <details className="fault-detail">
        <summary>{t("erreur.detail")}</summary>
        <pre>{fault.detail}</pre>
      </details>
    </div>
  );
}

type View = "livre" | "tri" | "planches" | "envoi";

/** In the book view, index -1 is the cover. */
const COVER = -1;

export default function App() {
  const [opened, setOpened] = useState<OpenedAlbum | null>(null);
  const [hist, setHist] = useState<History | null>(null);
  const [savedAlbum, setSavedAlbum] = useState<Album | null>(null);
  const [index, setIndex] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<Fault | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [building, setBuilding] = useState<string[] | null>(null);
  // The end-of-build report; the book opens behind it, it dismisses once.
  const [bilan, setBilan] = useState<BuildBilan | null>(null);
  // The proposals composed beside the album, and which one is on screen.
  // Both die with the report screen: past it there is one album.
  const [variantes, setVariantes] = useState<VarianteResume[]>([]);
  const [variante, setVariante] = useState<string | null>(null);
  // Index into the sorting view's entries while the keyboard review is up.
  const [revue, setRevue] = useState<number | null>(null);
  const [busyTitle, setBusyTitle] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  // A print PDF left the machine for this album: the Envoi screen then
  // offers the verdict form (the two questions of the launch protocol).
  const [exporte, setExporte] = useState(false);
  const [view, setView] = useState<View>("livre");
  const [curation, setCuration] = useState<Discard[]>([]);
  const [triSelected, setTriSelected] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [overflow, setOverflow] = useState<string | null>(null);
  // The supplier the destination screen reads. One per session:
  // the album carries no printer, the file it produces does.
  const [profil, setProfil] = useState("cloudprinter");
  // Loaded once and shared: the cover editor draws its sheet for the same
  // supplier the destination screen preflights against.
  const [printers, setPrinters] = useState<Printer[] | null>(null);
  // The caption proposed for the spread on screen, when its field is empty:
  // grey in place, Tab accepts, any other gesture ignores it. Never saved
  // by itself, never fetched twice for the same spread.
  const [proposition, setProposition] = useState<string | null>(null);
  // The keyboard cheat-sheet overlay (⌘/, menu Aide).
  const [shortcuts, setShortcuts] = useState(false);
  // Le panneau « Changer de format ». Il ne détient rien : le moteur rend un
  // album et un bilan, l'aperçu se lit, et l'appliquer passe par `apply` —
  // donc par l'historique, donc par ⌘Z. Rien n'atteint le disque avant ⌘S.
  const [bascule, setBascule] = useState(false);
  const [basculeFormats, setBasculeFormats] = useState<FormatPreset[]>([]);
  const [basculeChoisi, setBasculeChoisi] = useState<string | null>(null);
  const [basculeApercu, setBasculeApercu] = useState<{
    album: Album;
    bilan: BasculeBilan;
  } | null>(null);
  const [basculeEnCours, setBasculeEnCours] = useState(false);
  // La police de l'album, dans le même panneau : le format et la police sont
  // les deux propriétés qui changent tout sans rien recomposer. La liste est
  // chargée une seule fois, comme celle des formats — 787 faces sur un Mac
  // de série, et le filtre est ce qui rend ça praticable.
  const [polices, setPolices] = useState<PoliceOfferte[]>([]);
  const [policeFiltre, setPoliceFiltre] = useState("");
  const [policeInfo, setPoliceInfo] = useState<PoliceEtat | null>(null);

  // The report panel (Aide → Signaler), one of the three issue variants.
  const [signaler, setSignaler] = useState<SignalKind | null>(null);
  // The storage panel (Fichier → Stockage…): what the app wrote on the disk.
  const [stockage, setStockage] = useState(false);
  // The faithful preview: the book view reads the PDF instead of drawing it.
  // `pdfCle` counts re-renders, so pdf.js drops a document it has parsed.
  const [fidele, setFidele] = useState(false);
  // À propos : version, licence, et les trois actifs sous licence tierce.
  const [apropos, setApropos] = useState(false);
  // Préférences (⌘,) : la langue, et une note sur l'apparence.
  const [prefs, setPrefs] = useState(false);
  // Une mise à jour disponible, quand il y en a une. Jamais téléchargée
  // toute seule : le bandeau attend un clic, et se referme sans en attendre.
  const [maj, setMaj] = useState<Awaited<ReturnType<typeof checkUpdate>>>(null);
  const [majEnCours, setMajEnCours] = useState(false);
  const [pdfCle, setPdfCle] = useState(0);
  // La feuille de l'aperçu fidèle, quand il y en a une à l'écran. Le clavier
  // lui propose le tour avant de changer la planche sèchement : le geste n'est
  // jamais le seul chemin, et les deux chemins doivent donner le même livre.
  const feuilletage = useRef<Tourneur | null>(null);
  // The recent-albums list: welcome screen and Fichier menu read it.
  const [recents, setRecents] = useState<RecentAlbum[]>(readRecents);

  useEffect(() => {
    listPrinters().then(setPrinters, () => setPrinters([]));
  }, []);

  // Une seule interrogation, au lancement, en arrière-plan. Hors ligne, feed
  // injoignable, signature refusée : tout cela rend null, et l'app ne dit
  // rien. Personne n'a demandé, et une app qui ne joint pas GitHub est une
  // app qui marche.
  useEffect(() => {
    checkUpdate().then(setMaj, () => {});
  }, []);

  const album = hist?.album ?? null;
  const total = album?.spreads.length ?? 0;
  const dirty = album !== null && album !== savedAlbum;

  // Le dump courant suit la page de l'album, pas seulement celle de son
  // ouverture. Un ⌘Z sur une bascule ramène l'ancien format sous un dump
  // qui décrit le nouveau : les rectangles se retrouvent quand même, la
  // recherche retombant sur le cache par format, mais `geometrieCourante`
  // rendrait l'autre page. Le dump est forcément déjà là — l'album affiché
  // a été dessiné une fois —, donc rien ne se charge ici, on repointe.
  useEffect(() => {
    if (!album) return;
    adopterGeometrie(album.trim_mm, album.bleed_mm);
  }, [album]);

  // The adjustments store is a reading mirror, and App is its single truth:
  // after any album change — opening, an edit, ⌘Z, a bascule, a
  // recomposition, closing — the whole table is re-posed. A component
  // writing a committed réglage into the store instead of through
  // `edits.ts` would make ⌘Z lie; this effect is what keeps that invariant
  // cheap to hold.
  useEffect(() => {
    poserReglages(album?.reglages);
  }, [album]);

  // La face de l'album atteint le navigateur, et c'est la même que celle du
  // PDF : des octets rendus par une commande, jamais un nom de police
  // installée. Elle se recharge à l'ouverture et à chaque changement de
  // choix — y compris un ⌘Z, qui remet le champ à ce qu'il était.
  //
  // Le fichier manquant ne casse rien et ne se tait pas : le moteur retombe
  // sur la face du projet, l'export réussira, et l'écran le dit ici. Un livre
  // imprimé dans une police que personne n'a choisie sans le savoir est
  // exactement ce que ce projet refuse.
  const policeFichier = album?.police?.fichier;
  // La clé du rechargement n'est pas le nom du fichier : deux faces
  // différentes s'écrivent sous le même `police.ttf`, et s'arrêter au nom
  // laisserait l'écran mesurer la face d'avant pendant que l'album nomme
  // la suivante.
  const policeCle = album?.police
    ? `${album.police.fichier}|${album.police.postscript}|${album.police.nom}`
    : "";
  useEffect(() => {
    if (!opened) return;
    let vivant = true;
    void (async () => {
      try {
        const [octets, etat] = await Promise.all([
          policeOctets(policeFichier),
          policeEtat(policeFichier),
        ]);

        if (!vivant) return;
        await chargerFace(octets);
        if (!vivant) return;
        setPoliceInfo(etat);
        if (etat.manquante) setStatus(t("police.manquante"));
      } catch {
        // Une face illisible n'empêche pas d'ouvrir l'album : la pile de
        // `font.ts` retombe sur celle du moteur, qui est celle du PDF.
      }
    })();
    return () => {
      vivant = false;
    };
    // policeFichier est lu dans l'effet, policeCle est ce qui le déclenche.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opened, policeCle]);



  // Deux chronos de rendu, dev seulement (`mesure.ts`), pris avant le port en
  // Canvas pour qu'après il y ait quelque chose à comparer. Ils se ferment
  // dans un effet du parent : les effets des enfants tournent d'abord, donc la
  // planche est commitée quand celui-ci s'exécute, et le double `rAF` de
  // `jusquAuRendu` attend le pixel plutôt que la trame d'avant.
  const finPremiere = useRef<(() => void) | null>(null);
  const finPlanche = useRef<(() => void) | null>(null);
  const indexMesure = useRef<number | null>(null);
  // Comparé au rendu plutôt que dans un effet : ce qu'on veut chronométrer
  // est le travail d'affichage, pas le trajet de la frappe jusqu'à lui, et ce
  // trajet-là ne bougera pas d'un port de rendu. Le double rendu de
  // StrictMode ne relance rien, la ref ayant déjà pris la valeur.
  if (indexMesure.current !== index) {
    indexMesure.current = index;
    finPlanche.current = jusquAuRendu("planche.suivante");
  }
  useEffect(() => {
    finPremiere.current?.();
    finPremiere.current = null;
    finPlanche.current?.();
    finPlanche.current = null;
  });

  const adopt = useCallback((result: OpenedAlbum) => {
    finPremiere.current = jusquAuRendu("planche.premiere");
    resetThumbs();
    setOpened(result);
    setHist({ album: result.album, past: [], future: [] });
    setSavedAlbum(result.album);
    setIndex(0);
    setSelected(null);
    setTriSelected(null);
    setView("livre");
    setError(null);
    setStatus(null);
    setBilan(null);
    setFidele(false);
    forgetPdfs();
    // The browser harness's « __dev__ » album never enters the list.
    if (inTauri && result.dir) {
      setRecents(pushRecent({ dir: result.dir, title: result.album.title }));
    }
  }, []);

  /** Open one of the recent albums, by its remembered path. */
  const openRecent = useCallback(
    async (dir: string) => {
      try {
        const result = await openAlbumAt(dir);
        adopt(result);
        setCuration(await fetchCuration().catch(() => []));
      } catch (e) {
        setError(
          fault(t("erreur.reouverture"), e),
        );
      }
    },
    [adopt],
  );

  const openAlbum = useCallback(async () => {
    const picked = await pickAlbumFolder();
    if (picked === null) return;
    try {
      const result = await openAlbumAt(picked);
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
    } catch (e) {
      setError(fault(t("erreur.ouverture"), e));
    }
  }, [adopt]);

  /** Push an edited album onto the history. No-op edits stay off the stack. */
  const apply = useCallback((edit: (album: Album) => Album) => {
    setHist((h) => {
      if (!h) return h;
      const next = edit(h.album);
      if (next === h.album) return h;
      return {
        album: next,
        past: [...h.past.slice(-(HISTORY_CAP - 1)), h.album],
        future: [],
      };
    });
  }, []);

  const undo = useCallback(() => {
    setSelected(null);
    setHist((h) => {
      if (!h || h.past.length === 0) return h;
      return {
        album: h.past[h.past.length - 1],
        past: h.past.slice(0, -1),
        future: [h.album, ...h.future],
      };
    });
  }, []);

  const redo = useCallback(() => {
    setSelected(null);
    setHist((h) => {
      if (!h || h.future.length === 0) return h;
      return {
        album: h.future[0],
        past: [...h.past.slice(-(HISTORY_CAP - 1)), h.album],
        future: h.future.slice(1),
      };
    });
  }, []);

  // Saving reads the freshest history through a ref: ⌘S from inside a
  // caption field blurs the field first, and the commit that blur applies
  // must be part of what lands on disk.
  const histRef = useRef<History | null>(null);
  histRef.current = hist;
  // The report screen's own state, read from callbacks that must not be
  // rebuilt every time a proposal is swapped in.
  const fideleRef = useRef(false);
  fideleRef.current = fidele;
  const profilRef = useRef(profil);
  profilRef.current = profil;
  const bilanRef = useRef<BuildBilan | null>(null);
  const variantesRef = useRef<VarianteResume[]>([]);
  variantesRef.current = variantes;
  const save = useCallback(async () => {
    const h = histRef.current;
    if (!h) return false;
    try {
      await saveAlbum(h.album);
      setSavedAlbum(h.album);
      setStatus(t("etat.enregistre"));
      return true;
    } catch (e) {
      setError(fault(t("erreur.enregistrement"), e));
      return false;
    }
  }, []);

  /**
   * Turn the faithful preview on or off. Entering it re-renders the PDF when
   * the album has moved since the last one: the whole point of this mode is
   * that what is on screen is the file, so showing a stale file would be
   * worse than showing the DOM.
   *
   * The cover is its own render, per printer profile: the spine width comes
   * from the supplier, and a preview of somebody else's spine is a lie.
   */
  const basculerFidele = useCallback(async () => {
    if (fideleRef.current) {
      setFidele(false);
      return;
    }
    setRendering(true);
    setStatus(t("fidele.rendu"));
    try {
      // The browser harness has no engine: it shows whatever PDF the dev
      // album folder holds, which is the right thing to work the mode on and
      // the wrong thing to trust, so it says so.
      if (inTauri) {
        if (!(await save())) return;
        await renderPdf();
        // The cover is its own file and its own render. A failure there (a
        // photo folder that moved, say) must not cost the whole preview: the
        // interior is what the mode is mostly about.
        await renderCoverPreview(profilRef.current).catch(() => {});
      }
      forgetPdfs();
      setPdfCle((k) => k + 1);
      setFidele(true);
      setStatus(
        inTauri
          ? t("fidele.pret")
          : t("fidele.harnais"),
      );
    } catch (e) {
      setError(fault(t("erreur.fidele"), e));
    } finally {
      setRendering(false);
    }
  }, [save]);

  /**
   * Show another of the proposals composed from the same photos. The album on
   * screen is swapped whole, curation included: a tighter book sets more
   * photos aside, and the sorting view has to describe the album that is
   * open. Reversible until the first save, which takes the others away.
   */
  const basculerVariante = useCallback(
    async (id: string | null) => {
      try {
        const result = await chooseVariante(id ?? "demandee");
        // `adopt` clears the report screen, which is precisely the screen the
        // user is standing on: put it back, with the count of the proposal
        // now open. What was read and how many chapters do not change from
        // one proposal to the next; how many photos were kept does.
        const base = bilanRef.current;
        adopt(result);
        setCuration(await fetchCuration().catch(() => []));
        setVariante(id);
        const v = id ? variantesRef.current.find((x) => x.id === id) : null;
        if (base) {
          setBilan(v ? { ...base, photos_kept: v.photos } : base);
        }
      } catch (e) {
        setError(fault(t("erreur.variante"), e));
      }
    },
    [adopt],
  );

  /**
   * Put the colophon page in or take it out, from the Envoi screen. The page
   * is an ordinary spread at the end of the book, so this is an ordinary
   * edit: ⌘Z undoes it, ⌘S saves it, and the preflight recounts the pages on
   * its own. The text is rendered by the engine from the facts the album
   * carries; the window never writes that page itself.
   */
  const toggleColophon = useCallback(
    async (on: boolean) => {
      const current = histRef.current?.album;
      if (!current) return;
      try {
        const spread = on ? await colophonSpread(current) : null;
        if (on && !spread) {
          setStatus(
            t("etat.colophon.trop.vieux"),
          );
          return;
        }
        apply((a) => setColophon(a, spread));
        setStatus(
          on
            ? t("etat.colophon.ajoute")
            : t("etat.colophon.retire"),
        );
      } catch (e) {
        setError(fault(t("erreur.colophon"), e));
      }
    },
    [apply],
  );

  /**
   * Put the half-title in or take it out, from the Envoi screen. Same
   * mechanics as the colophon page at the other end of the book: an
   * ordinary spread, an ordinary edit, ⌘Z and ⌘S included, and the text
   * comes from the engine rather than from the window.
   */
  const toggleGarde = useCallback(
    async (on: boolean) => {
      const current = histRef.current?.album;
      if (!current) return;
      try {
        const spread = on ? await gardeSpread(current) : null;
        if (on && !spread) {
          setStatus(
            t("etat.garde.trop.vieux"),
          );
          return;
        }
        apply((a) => setGarde(a, spread));
        setStatus(
          on
            ? t("etat.garde.ajoutee")
            : t("etat.garde.retiree"),
        );
      } catch (e) {
        setError(fault(t("erreur.garde"), e));
      }
    },
    [apply],
  );

  const regenPdf = useCallback(async () => {
    if (rendering || !hist || !(await save())) return;
    setRendering(true);
    setStatus(t("export.rendu"));
    try {
      // Une fois par photo à l'écran serait bavard pour une région vivante :
      // le compteur avance au plus une fois par seconde, et la dernière
      // photo passe toujours.
      let dernier = 0;
      const written = await exportPdf(hist.album.title, profil, (done, total) => {
        const now = performance.now();
        if (done < total && now - dernier < 1000) return;
        dernier = now;
        setStatus(t("export.progress", { done, total }));
      });
      // What was actually written, named. A supplier who wants two files gets
      // two, and the second one is the thing nobody thinks to look for.
      setStatus(
        written === null
          ? t("export.enregistrement.annule")
          : written.length > 1
            ? t("export.fichiers", { n: written.length, liste: written.join(" · ") })
            : t("export.pdf", { nom: written[0] }),
      );
      if (written !== null) setExporte(true);
    } catch (e) {
      if (String(e).includes("export annulé")) {
        setStatus(t("export.annule"));
      } else {
        setError(fault(t("erreur.export"), e));
      }
    } finally {
      setRendering(false);
    }
  }, [save, rendering, hist, profil]);

  /** Build an album from a photo folder, streaming the engine's progress. */
  const createAlbum = useCallback(async (
    dir: string,
    format: string,
    spreads: number,
    densite: string,
    title: string | null,
  ) => {
    setBuilding([]);
    setBusyTitle(null);
    setError(null);
    // Every line is kept: the counter lines drive the progress bar, the
    // named stages feed the visible log.
    const off = await onBuildProgress((line) =>
      setBuilding((b) => (b ? [...b, line] : [line])),
    );
    try {
      const result = await buildAlbum(dir, format, spreads, densite, title);
      adopt(result.opened);
      setCuration(await fetchCuration().catch(() => []));
      setBilan(result.bilan);
      bilanRef.current = result.bilan;
      setVariantes(result.variantes ?? []);
      setVariante(null);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("annulée")) setStatus(t("compo.annulee"));
      else if (msg.includes("aucune photo exploitable"))
        // The engine refused rather than open an empty album. The gesture
        // that gets the user out is choosing another folder, so say so.
        setError(
          fault(t("compo.vide"), e),
        );
      else setError(fault(t("erreur.compo"), e));
    } finally {
      off();
      setBuilding(null);
    }
  }, [adopt]);

  /**
   * Recompose the album in place. Edited and locked spreads survive
   * verbatim, so the button is safe; it still resets ⌘Z, which is the one
   * thing worth a confirmation.
   */
  const recompose = useCallback(async () => {
    if (!hist || building) return;
    if (
      !(await confirmDialog(
        t("recomp.confirme"),
      ))
    ) {
      return;
    }
    if (dirty && !(await save())) return;
    setBusyTitle(hist.album.title);
    setBuilding([]);
    const off = await onBuildProgress((line) =>
      setBuilding((b) => (b ? [...b, line] : [line])),
    );
    try {
      const result = await recomposeAlbum();
      adopt(result);
      setCuration(await fetchCuration().catch(() => []));
      setStatus(t("recomp.ok"));
    } catch (e) {
      if (String(e).includes("annulée")) setStatus(t("recomp.annulee"));
      else setError(fault(t("erreur.recomp"), e));
    } finally {
      off();
      setBuilding(null);
      setBusyTitle(null);
    }
  }, [hist, building, dirty, save, adopt]);

  /** Ouvrir le panneau de format, en chargeant la liste une seule fois.
   *
   *  Enregistrer d'abord, comme la recomposition : le moteur bascule
   *  `album.json`, donc l'album tel qu'il est sur le disque. Avec des
   *  retouches non enregistrées, l'aperçu porterait sur une version
   *  périmée et l'appliquer les effacerait sans rien dire — précisément la
   *  perte silencieuse que cette vague existe pour empêcher. */
  const ouvrirBascule = useCallback(async () => {
    if (dirty && !(await save())) return;
    setBasculeChoisi(null);
    setBasculeApercu(null);
    // Le panneau s'ouvre neuf, filtre compris : rouvrir sur « Aucune police
    // ne correspond » et un mot tapé il y a une heure se lit comme une panne.
    setPoliceFiltre("");
    setBascule(true);

    if (basculeFormats.length === 0) listFormats().then(setBasculeFormats, () => {});
    // Les faces de la machine, une fois par ouverture du panneau plutôt
    // qu'une fois par session : une police installée entre-temps doit
    // apparaître, et la marche des dossiers coûte quelques dizaines de
    // millisecondes.
    if (inTauri) polices_installees().then(setPolices, () => {});
  }, [basculeFormats.length, dirty, save]);

  /** Choisir une face : le moteur la copie dans le dossier de l'album, et
   *  l'album la nomme. Par `apply`, donc annulable par ⌘Z comme la bascule
   *  de format — le fichier reste à côté, l'album ne le nomme plus. */
  const choisirLaPolice = useCallback(
    async (offerte: PoliceOfferte) => {
      try {
        const police = await choisirPolice(offerte.rang);
        apply((a) => ({ ...a, police }));
        setStatus(t("police.choisie", { nom: nomLisible(offerte) }));
      } catch (e) {
        setError(fault(t("erreur.police"), e));
      }
    },
    [apply],
  );

  /** Revenir à la face du moteur. Le fichier posé reste sur le disque : il
   *  ne pèse rien, un autre choix l'écrase, et l'effacer derrière un ⌘Z qui
   *  peut se ⇧⌘Z serait le seul geste destructeur du panneau. */
  const rendreLaPolice = useCallback(() => {
    apply((a) => {
      if (!a.police) return a;
      const { police: _, ...reste } = a;
      return reste as Album;
    });
    setStatus(t("police.rendue"));
  }, [apply]);


  /** Demander au moteur ce que ce format donnerait. Il n'écrit rien.
   *
   *  Et charger la géométrie de ce format-là, dans le fond perdu de
   *  l'album : chaque rectangle de l'écran sort du dump du moteur, celui
   *  de l'album ouvert décrit l'ancienne page, et une page dont personne
   *  n'a chargé la géométrie ne se dessine pas — elle jette, et sans
   *  frontière d'erreur au-dessus de l'arbre, une fenêtre blanche. Le
   *  dump vient donc *avant* que le bilan s'affiche : « Appliquer » ne
   *  s'offre que quand il y a de quoi dessiner ce qu'il applique. */
  const apercuBascule = useCallback(
    async (f: FormatPreset) => {
      setBasculeChoisi(f.name);
      setBasculeApercu(null);
      setBasculeEnCours(true);
      try {
        const apercu = await basculeAlbum(f.w, f.h, profilRef.current);
        const { trim_mm, bleed_mm } = apercu.album;
        await chargeGeometrieFormat(trim_mm.w, trim_mm.h, bleed_mm);
        setBasculeApercu(apercu);
      } catch (e) {
        setBascule(false);
        setError(fault(t("erreur.bascule"), e));
      } finally {
        setBasculeEnCours(false);
      }
    },
    [],
  );

  /** Appliquer l'aperçu. Par `apply`, donc annulable par ⌘Z comme une
   *  retouche : c'est pour ça que le moteur rend un album au lieu d'en
   *  enregistrer un.
   *
   *  La géométrie du nouveau format est adoptée d'abord, dans le même
   *  souffle : l'album change de page à l'instruction suivante, et un
   *  rendu entre les deux dessinerait l'ancienne. `adopterGeometrie`
   *  rend faux quand le dump manque, et alors rien ne s'applique — un
   *  album qu'on ne sait pas dessiner ne monte pas à l'écran. */
  const appliquerBascule = useCallback(() => {
    if (!basculeApercu) return;
    const { album } = basculeApercu;
    if (!adopterGeometrie(album.trim_mm, album.bleed_mm)) {
      setBascule(false);
      setError(fault(t("erreur.bascule"), t("bascule.geometrie")));
      return;
    }
    apply(() => album);
    setBascule(false);
    setStatus(t("bascule.faite", { w: album.trim_mm.w, h: album.trim_mm.h }));
  }, [basculeApercu, apply]);

  /** Back to the creation screen. Unsaved work asks before dying. */
  const closeAlbum = useCallback(async () => {
    if (
      dirty &&
      !(await confirmDialog(
        t("fermer.confirme"),
      ))
    ) {
      return;
    }
    setHist(null);
    setOpened(null);
    setSavedAlbum(null);
    setCuration([]);
    setSelected(null);
    setTriSelected(null);
    setView("livre");
    setStatus(null);
    setError(null);
  }, [dirty]);

  /** Bring a discarded photo back, next to the photo that beat it. */
  const rescue = useCallback(
    (entry: TriEntry) => {
      if (!album) return;
      const anchorByWinner = entry.kept ? spreadOf(album, entry.kept) : -1;
      const anchor = anchorByWinner >= 0 ? anchorByWinner : Math.max(index, 0);
      const result = rescuePhoto(
        album,
        { src: entry.src, focal: entry.focal },
        anchor,
      );
      if (!result) {
        setStatus(
          t("repeche.place", { n: anchor + 1 }),
        );
        return;
      }
      setTriSelected(null);
      apply(() => result.album);
      setIndex(result.at);
      setStatus(t("repeche.ok", { n: result.at + 1 }));
    },
    [album, index, apply],
  );

  /** A drawer photo lands in a case; the displaced one joins the drawer. */
  const place = useCallback(
    (slot: number, photo: Slot) => {
      if (!album || index < 0) return;
      const before = album.spreads[index]?.slots[slot]?.src;
      const next = placePhoto(album, index, slot, photo);
      if (next === album) {
        setStatus(t("place.doublon"));
        return;
      }
      apply(() => next);
      setStatus(
        before
          ? t("place.remplacee")
          : t("place.ok"),
      );
    },
    [album, index, apply],
  );

  /** Change view the one canonical way: selections drop, and only the book
   *  view knows the cover. */
  const gotoView = useCallback(
    (v: View) => {
      setView(v);
      setSelected(null);
      setTriSelected(null);
      if (v !== "livre" && index === COVER) setIndex(0);
      // The clicked tab keeps focus and would eat the view's own keys,
      // Enter first among them: give the keyboard back to the view.
      (document.activeElement as HTMLElement | null)?.blur?.();
    },
    [index],
  );

  /** True when the keyboard focus sits in a text field: the field owns its
   *  own undo history and its own editing keys. */
  const inField = () => {
    const el = document.activeElement;
    return el !== null && /^(INPUT|TEXTAREA)$/.test(el.tagName);
  };

  // The quick template cycle: G walks the spread through the layouts its
  // photos can take right now, engine-judged (count and orientation, the
  // same rule the picker filters on), ⇧G walks back. Exact capacity only:
  // the cycle never drops a photo; the smaller layouts stay in the picker.
  const cycleGabarit = (sens: 1 | -1) => {
    if (!album || view !== "livre" || index < 0) return;
    const spread = album.spreads[index];
    if (!spread || templateCapacity(spread.template) === 0) return;
    void (async () => {
      const notes = await gabaritsCompatibles(spread.slots.map((s) => s.src));
      // Engine unreachable: cycle the count-compatible list, which is the
      // same honest fallback the picker shows.
      const offerts: [string, number][] =
        notes ?? templateChoices(spread).map(([nom]) => [nom, 1]);
      // Same entries as the picker — one per arrangement — so G and the
      // panel walk one list and not two: exact capacity only, the cycle
      // never drops a photo, the smaller layouts stay in the picker.
      const choix = choixOfferts(offerts, spread.template).filter(
        (c) => c.capacite === spread.slots.length,
      );
      const f = formeDe(spread.template);
      const courante = f ? cleDeForme(f) : "";
      const at = choix.findIndex((c) => c.cle === courante);
      const suivante = choix[(at + sens + choix.length) % choix.length];
      if (!suivante || suivante.cle === courante) return;
      apply((a) => changeTemplate(a, index, faceFor(suivante.template, index)));
      setStatus(t("gabarit.cycle", { nom: suivante.libelle }));
    })();
  };

  // Every command of the app, by name. The window's keydown handler and the
  // native menu both land here, so a chord and its menu item are one code
  // path with one guard each.
  const raw: Record<string, () => void> = {
    nouveau: () => void closeAlbum(),
    ouvrir: () => void openAlbum(),
    enregistrer: () => {
      // Blur commits the field being edited; save once that landed.
      const el = document.activeElement as HTMLElement | null;
      if (el && inField()) {
        el.blur();
        setTimeout(() => void save(), 0);
      } else {
        void save();
      }
    },
    exporter: () => {
      if (album) gotoView("envoi");
    },
    fermerAlbum: () => void closeAlbum(),
    annuler: () => {
      if (inField()) document.execCommand("undo");
      else undo();
    },
    retablir: () => {
      if (inField()) document.execCommand("redo");
      else redo();
    },
    "vue-livre": () => album && gotoView("livre"),
    "vue-tri": () => album && gotoView("tri"),
    "vue-planches": () => album && gotoView("planches"),
    "vue-envoi": () => album && gotoView("envoi"),
    couverture: () => {
      if (!album) return;
      setView("livre");
      setSelected(null);
      setTriSelected(null);
      setIndex(COVER);
    },
    revue: () => {
      if (!album) return;
      const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
      if (!entries.length) return;
      setView("tri");
      setStatus(null);
      setRevue(0);
    },
    reserve: () => {
      if (!album) return;
      setView("livre");
      setDrawerOpen((o) => !o);
    },
    gabarit: () => {
      if (album && view === "livre" && index >= 0) {
        window.dispatchEvent(new Event("colophon:gabarit"));
      }
    },
    "gabarit-suivant": () => cycleGabarit(1),
    "gabarit-precedent": () => cycleGabarit(-1),
    dupliquer: () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      apply((a) => duplicateSpread(a, index));
      setIndex(index + 1);
      setStatus(t("planche.dupliquee", { n: index + 1 }));
    },
    figer: () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      const was = album.spreads[index]?.locked;
      apply((a) => toggleLock(a, index));
      setStatus(
        was
          ? t("planche.liberee")
          : t("planche.figee.status"),
      );
    },
    // The way out of the lock. Asks first: it throws away hand work, and
    // the spread it gives back may be nothing like the one on screen.
    "rendre-auto": () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      const spread = album.spreads[index];
      if (!spread) return;
      void (async () => {
        try {
          const origin = await originSpread(album, index);
          if (!origin) {
            setStatus(
              t("etat.auto.insertion", { n: index + 1 }),
            );
            return;
          }
          const ok = await confirmDialog(
            t("auto.confirme", { n: index + 1 }),
          );
          if (!ok) return;
          apply((a) => restoreSpread(a, index, origin));
          setStatus(t("etat.auto.rendue", { n: index + 1 }));
        } catch (e) {
          setError(
            fault(t("erreur.auto"), e),
          );
        }
      })();
    },
    "inserer-vide": () => {
      if (!album || view !== "livre" && view !== "planches") return;
      const at = Math.max(index, 0);
      apply((a) => insertSpread(a, at, "vide"));
      setIndex(at + 1);
      setStatus(t("planche.vide.inseree"));
    },
    "inserer-texte": () => {
      if (!album || view !== "livre" && view !== "planches") return;
      const at = Math.max(index, 0);
      apply((a) => insertSpread(a, at, "texte"));
      setIndex(at + 1);
      setStatus(t("planche.texte.inseree"));
    },
    "supprimer-planche": () => {
      if (!album || index < 0 || (view !== "livre" && view !== "planches")) return;
      apply((a) => removeSpread(a, index));
      setStatus(t("planche.supprimee", { n: index + 1 }));
    },
    // The four overlays are one place at a time: opening one closes the
    // others. Two panels stacked, neither trapping the keyboard, is how
    // Échap stops doing what it says.
    raccourcis: () => {
      setStockage(false);
      setApropos(false);
      setSignaler(null);
      setPrefs(false);
      setBascule(false);
      setShortcuts((s) => !s);
    },
    stockage: () => {
      setShortcuts(false);
      setApropos(false);
      setSignaler(null);
      setPrefs(false);
      setStockage((s) => !s);
    },
    apropos: () => {
      setShortcuts(false);
      setStockage(false);
      setSignaler(null);
      setPrefs(false);
      setApropos((a) => !a);
    },
    preferences: () => {
      setShortcuts(false);
      setStockage(false);
      setSignaler(null);
      setApropos(false);
      setPrefs((p) => !p);
    },
    // The faithful preview: only in the book view, where there is a spread
    // to be faithful about.
    fidele: () => {
      if (!album || view !== "livre") return;
      void basculerFidele();
    },
    // The three report variants. A bug needs nothing; the two layout
    // complaints quote the spread on screen, the crop one its selected cell.
    "signaler-bug": () => {
      setPrefs(false);
      setShortcuts(false);
      setStockage(false);
      setApropos(false);
      setSignaler("bug");
    },
    "signaler-planche": () => {
      if (!album || index < 0) {
        setStatus(t("signal.planche.dabord"));
        return;
      }
      setPrefs(false);
      setSignaler("planche");
    },
    "signaler-recadrage": () => {
      if (!album || index < 0 || selected === null) {
        setStatus(t("signal.case.dabord"));
        return;
      }
      setPrefs(false);
      setSignaler("recadrage");
    },
  };
  const rawRef = useRef(raw);
  rawRef.current = raw;

  // A chord can reach the app twice, once through the window's keydown and
  // once through the menu accelerator, depending on how WebKit routes key
  // equivalents. Whoever speaks second within the window is the same
  // keypress: one action runs.
  const lastFire = useRef({ action: "", source: "", t: 0 });
  const fire = useCallback((source: "kbd" | "menu", action: string) => {
    const now = performance.now();
    const l = lastFire.current;
    if (l.action === action && l.source !== source && now - l.t < 150) return;
    lastFire.current = { action, source, t: now };
    rawRef.current[action]?.();
  }, []);

  // The browser harness has no native menu: a window event stands in for
  // the three Aide → Signaler items, the way the gabarit picker is asked
  // for. Same table, same guards.
  useEffect(() => {
    const onSignal = (e: Event) => {
      const kind = (e as CustomEvent<string>).detail;
      if (kind === "bug" || kind === "planche" || kind === "recadrage") {
        fire("menu", `signaler-${kind}`);
      }
    };
    const onStockage = () => fire("menu", "stockage");
    window.addEventListener("colophon:signaler", onSignal);
    window.addEventListener("colophon:stockage", onStockage);
    return () => {
      window.removeEventListener("colophon:signaler", onSignal);
      window.removeEventListener("colophon:stockage", onStockage);
    };
  }, [fire]);

  // The native menu follows the app state: rebuilt when the album opens or
  // closes and when the recents change, cheap both times.
  const openRecentRef = useRef(openRecent);
  openRecentRef.current = openRecent;
  const albumOpen = album !== null;
  // One subscription for the whole tree: a language change re-renders App,
  // and every component below re-reads its t() calls on that render.
  const lang = useLangue();
  useEffect(() => {
    if (!inTauri) return;
    const actions: MenuActions = {
      nouveau: () => fire("menu", "nouveau"),
      ouvrir: () => fire("menu", "ouvrir"),
      ouvrirRecent: (dir) => void openRecentRef.current(dir),
      enregistrer: () => fire("menu", "enregistrer"),
      exporter: () => fire("menu", "exporter"),
      fermerAlbum: () => fire("menu", "fermerAlbum"),
      stockage: () => fire("menu", "stockage"),
      apropos: () => fire("menu", "apropos"),
      preferences: () => fire("menu", "preferences"),
      apercuFidele: () => fire("menu", "fidele"),
      annuler: () => fire("menu", "annuler"),
      retablir: () => fire("menu", "retablir"),
      vue: (v) => fire("menu", `vue-${v}`),
      couverture: () => fire("menu", "couverture"),
      revue: () => fire("menu", "revue"),
      reserve: () => fire("menu", "reserve"),
      gabarit: () => fire("menu", "gabarit"),
      dupliquer: () => fire("menu", "dupliquer"),
      figer: () => fire("menu", "figer"),
      rendreAuto: () => fire("menu", "rendre-auto"),
      insererVide: () => fire("menu", "inserer-vide"),
      insererTexte: () => fire("menu", "inserer-texte"),
      supprimerPlanche: () => fire("menu", "supprimer-planche"),
      raccourcis: () => fire("menu", "raccourcis"),
      signalerBug: () => fire("menu", "signaler-bug"),
      signalerPlanche: () => fire("menu", "signaler-planche"),
      signalerRecadrage: () => fire("menu", "signaler-recadrage"),
    };
    installMenu(() => actions, albumOpen, recents).catch(() => {
      // A shell without the menu permission still has the window shortcuts.
    });
  }, [albumOpen, recents, fire, lang]);

  // The faithful preview belongs to the book view and to the album it was
  // rendered from: leaving either drops it rather than showing a page of a
  // file nobody is looking at any more.
  useEffect(() => {
    if (view !== "livre") setFidele(false);
  }, [view]);

  // A removed spread can leave the position past the end.
  useEffect(() => {
    if (total > 0 && index >= total) setIndex(total - 1);
  }, [total, index]);

  // The selection belongs to one spread only.
  useEffect(() => setSelected(null), [index]);

  // Unsaved work guards the window, in the app and in the dev browser alike.
  useEffect(() => {
    if (!dirty) return;
    const guard = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);

  // Transient status line: every message expires the same way; errors have
  // their own banner and never travel through here.
  useEffect(() => {
    if (!status) return;
    const t = setTimeout(() => setStatus(null), 4000);
    return () => clearTimeout(t);
  }, [status]);

  // In a plain browser the album comes from the dev server: open it straight
  // away on arrival. Once only: « Nouveau » must reach the welcome screen in
  // the harness too, that is where the creation flow gets styled.
  const autoOpened = useRef(false);
  useEffect(() => {
    if (inTauri || opened || autoOpened.current) return;
    autoOpened.current = true;
    void openAlbum();
  }, [opened, openAlbum]);

  // Neighbouring spreads are fetched ahead so a page turn never flashes empty.
  useEffect(() => {
    if (!album) return;
    for (const i of [index + 1, index - 1, index + 2]) {
      album.spreads[i]?.slots.forEach((s) => loadThumb(s.src).catch(() => {}));
    }
  }, [album, index]);

  // Another album, another verdict: the offer dies with the album it judged.
  useEffect(() => {
    setExporte(false);
  }, [opened]);

  // The proposed caption of the spread on screen, fetched when its field is
  // empty. Any move drops it, a refusal leaves no trace, and it never enters
  // the album without Tab (`legende::proposition` writes the words).
  useEffect(() => {
    setProposition(null);
    if (!album || view !== "livre" || index < 0) return;
    const s = album.spreads[index];
    if (!s || s.caption || !s.slots.length) return;
    let alive = true;
    legendeProposee(index).then(
      (p) => alive && setProposition(p),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [album, index, view]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A focused control owns the keyboard: an input takes the letters, a
      // button takes space and enter (standard activation). App-level
      // chords still pass: ⌘S from inside a caption field must save.
      const cible = e.target as HTMLElement | null;
      const key = e.key.toLowerCase();
      // The end-of-build report holds the keyboard: Enter or Escape opens
      // the book, nothing may reach the editor behind it. Enter on a focused
      // button stays native, both its actions dismiss the report anyway.
      if (bilan) {
        if (
          e.key === "Escape" ||
          (e.key === "Enter" && !(cible && cible.tagName === "BUTTON"))
        ) {
          e.preventDefault();
          setBilan(null);
        }
        return;
      }
      // The keyboard review owns the plain keys, even from a focused button:
      // arrows browse, R rescues, X confirms the discard and moves on,
      // Escape leaves. ⌘-chords keep their app meaning below.
      if (view === "tri" && revue !== null && !e.metaKey && album) {
        const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
        const last = entries.length - 1;
        const i = Math.max(0, Math.min(revue, last));
        if (e.key === "Escape") {
          e.preventDefault();
          setRevue(null);
          return;
        }
        if (e.key === "ArrowLeft") {
          e.preventDefault();
          setStatus(null);
          setRevue(Math.max(0, i - 1));
          return;
        }
        if (e.key === "ArrowRight" || key === "x") {
          e.preventDefault();
          if (i >= last) {
            setRevue(null);
            setStatus(t("revue.terminee"));
          } else {
            setStatus(null);
            setRevue(i + 1);
          }
          return;
        }
        if (key === "r") {
          e.preventDefault();
          if (entries[i]) rescue(entries[i]);
          return;
        }
        return;
      }
      // Preferences hold the keyboard like the panels below them.
      if (prefs) {
        if (e.key === "Escape") {
          e.preventDefault();
          setPrefs(false);
        }
        return;
      }
      // À propos holds the keyboard like the panels below it.
      if (apropos) {
        if (e.key === "Escape") {
          e.preventDefault();
          setApropos(false);
        }
        return;
      }
      // The storage panel holds the keyboard like the two below it.
      if (stockage) {
        if (e.key === "Escape") {
          e.preventDefault();
          setStockage(false);
        }
        return;
      }
      // The report panel holds the keyboard the way the cheat-sheet does;
      // its focused controls keep their native keys.
      if (signaler) {
        if (e.key === "Escape") {
          e.preventDefault();
          setSignaler(null);
        }
        return;
      }
      // The shortcuts overlay holds the keyboard until it closes.
      if (shortcuts) {
        if (e.key === "Escape" || (e.metaKey && key === "/")) {
          e.preventDefault();
          setShortcuts(false);
        }
        return;
      }
      if (cible && /^(INPUT|SELECT|TEXTAREA|BUTTON)$/.test(cible.tagName)) {
        // A focused field keeps its letters and its own editing chords,
        // native ⌘Z included; a few app chords still pass.
        const appChord =
          e.metaKey && ["s", "o", "1", "2", "3", "4"].includes(key);
        if (!appChord) return;
        if (cible.tagName === "BUTTON") cible.blur();
      }
      // App chords: one name each, the same names the menu speaks, so a
      // keypress and its menu item are one code path.
      if (e.metaKey) {
        const chord =
          key === "n"
            ? "nouveau"
            : key === "o"
              ? "ouvrir"
              : key === "s"
                ? "enregistrer"
                : key === "z"
                  ? e.shiftKey
                    ? "retablir"
                    : "annuler"
                  : key === "e" && e.shiftKey
                    ? "exporter"
                    : key === "p" && e.shiftKey
                      ? "fidele"
                      : key === "d"
                      ? "dupliquer"
                      : key === "l"
                        ? "figer"
                        : key === ","
                        ? "preferences"
                        : key === "/"
                          ? "raccourcis"
                          : key === "1"
                            ? "vue-livre"
                            : key === "2"
                              ? "vue-tri"
                              : key === "3"
                                ? "vue-planches"
                                : key === "4"
                                  ? "vue-envoi"
                                  : null;
        if (chord) {
          e.preventDefault();
          fire("kbd", chord);
          return;
        }
      }
      // The destination screen has no spread under the cursor: it keeps the
      // global shortcuts and nothing else.
      if (view === "envoi") return;
      // The sorting view keeps the global shortcuts, plus Enter to start
      // the keyboard review; nothing spread-bound.
      if (view === "tri") {
        if (e.key === "Enter" && album) {
          e.preventDefault();
          const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
          if (entries.length) {
            const at = triSelected
              ? entries.findIndex((x) => x.src === triSelected)
              : 0;
            setStatus(null);
            setRevue(Math.max(0, at));
          }
          return;
        }
        if (e.key === "Escape") setTriSelected(null);
        return;
      }
      if (!total || !album) return;

      if (view === "planches") {
        if ((e.key === "Backspace" || e.key === "Delete") && index >= 0) {
          e.preventDefault();
          apply((a) => removeSpread(a, index));
          setStatus(t("planche.supprimee", { n: index + 1 }));
          return;
        }
        // Escape behaves like everywhere else: it lets go of the current
        // selection instead of being swallowed.
        if (e.key === "Escape") {
          setSelected(null);
          setTriSelected(null);
          return;
        }
        // The cover is the light table's page zero: ← reaches it.
        const step = (d: number) =>
          setIndex((i) => Math.min(total - 1, Math.max(COVER, i + d)));
        if (e.key === "ArrowRight") step(1);
        if (e.key === "ArrowLeft") step(-1);
        if (e.key === "Enter") setView("livre");
        return;
      }

      if (
        e.metaKey &&
        e.shiftKey &&
        (e.key === "ArrowRight" || e.key === "ArrowLeft") &&
        selected !== null &&
        index >= 0
      ) {
        e.preventDefault();
        const to = index + (e.key === "ArrowRight" ? 1 : -1);
        if (to < 0 || to >= total) return;
        const blocked = moveBlocker(album, index, selected, to);
        if (blocked === "target_full") {
          setStatus(t("move.pleine", { n: to + 1 }));
        } else if (blocked === "target_text") {
          setStatus(t("move.texte", { n: to + 1 }));
        } else if (blocked === "source_breaks") {
          setStatus(t("move.refuse"));
        } else if (blocked === null) {
          setSelected(null);
          apply((a) => movePhoto(a, index, selected, to));
          setStatus(t("move.ok", { n: to + 1 }));
        }
        return;
      }

      // Crop keys on the selected photo: ⌥flèches déplacent le cadrage,
      // + / − zooment, 0 revient au remplissage exact.
      if (selected !== null && index >= 0) {
        const slot = album.spreads[index]?.slots[selected];
        if (slot) {
          const zoom = slot.zoom ?? 1;
          if (e.altKey && e.key.startsWith("Arrow")) {
            e.preventDefault();
            const d = 0.02;
            const [fx, fy] = slot.focal;
            const focal: [number, number] =
              e.key === "ArrowLeft"
                ? [fx - d, fy]
                : e.key === "ArrowRight"
                  ? [fx + d, fy]
                  : e.key === "ArrowUp"
                    ? [fx, fy - d]
                    : [fx, fy + d];
            apply((a) => setSlotCrop(a, index, selected, focal, zoom));
            return;
          }
          if (e.key === "+" || e.key === "=") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, zoom + 0.1));
            return;
          }
          if (e.key === "-" || e.key === "_") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, zoom - 0.1));
            return;
          }
          if (e.key === "0") {
            e.preventDefault();
            apply((a) => setSlotCrop(a, index, selected, slot.focal, 1));
            setStatus(t("zoom.remis"));
            return;
          }
        }
      }

      if ((e.key === "Backspace" || e.key === "Delete") && selected !== null && index >= 0) {
        e.preventDefault();
        setSelected(null);
        apply((a) => removePhoto(a, index, selected));
        return;
      }
      // The proposed caption: Tab takes it, any other gesture ignores it.
      // One press, then Tab is a plain Tab again (the proposal is gone).
      // Never from a focused field: the guard above already returned.
      if (
        e.key === "Tab" &&
        !e.metaKey &&
        !e.altKey &&
        !e.ctrlKey &&
        proposition &&
        index >= 0
      ) {
        e.preventDefault();
        const p = proposition;
        apply((a) => setSpreadCaption(a, index, p));
        setStatus(t("legende.posee", { texte: p }));
        return;
      }
      if (e.key === "Escape") {
        setSelected(null);
        return;
      }
      if (key === "p" && !e.metaKey) {
        setDrawerOpen((o) => !o);
        return;
      }
      if (key === "g" && !e.metaKey && !e.altKey && !e.ctrlKey) {
        e.preventDefault();
        fire("kbd", e.shiftKey ? "gabarit-precedent" : "gabarit-suivant");
        return;
      }
      // La feuille d'abord : si l'aperçu fidèle est à l'écran et qu'elle peut
      // tourner, elle prend la commande et changera la planche au bout de son
      // mouvement. Sinon la planche change sèchement, comme avant — c'est
      // aussi ce que reçoit un lecteur qui a demandé moins de mouvement.
      const step = (d: number) => {
        if (feuilletage.current?.tourner(d)) return;
        setIndex((i) => Math.min(total - 1, Math.max(COVER, i + d)));
      };
      switch (e.key) {
        case "ArrowRight":
        case "ArrowDown":
        case " ":
        // Page haut et Page bas font ce que fait le coin de la feuille : le
        // geste n'est jamais le seul chemin, et un clavier de portable qui
        // n'a pas de pavé les porte quand même.
        case "PageDown":
          e.preventDefault();
          step(1);
          break;
        case "ArrowLeft":
        case "ArrowUp":
        case "PageUp":
          e.preventDefault();
          step(-1);
          break;
        case "Home":
          setIndex(0);
          break;
        case "End":
          setIndex(total - 1);
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    total,
    apply,
    index,
    selected,
    album,
    view,
    bilan,
    revue,
    curation,
    opened,
    rescue,
    triSelected,
    fire,
    shortcuts,
    signaler,
    stockage,
    apropos,
    prefs,
    proposition,
  ]);

  // The review dies with its subject: leaving the sorting view, or rescuing
  // the last photo, closes it. There is nothing left to review.
  useEffect(() => {
    if (revue === null) return;
    const left = album
      ? triEntries(album, curation, opened?.thumb_srcs ?? []).length
      : 0;
    if (view !== "tri" || left === 0) setRevue(null);
  }, [revue, view, album, curation, opened]);

  if (!album || building) {
    return (
      <>
        <Empty
          onOpen={openAlbum}
          onCreate={createAlbum}
          building={building}
          busyTitle={busyTitle}
          error={error}
          onDismissError={() => setError(null)}
          onCancelBuild={() => void cancelBuild()}
          recents={inTauri ? recents : []}
          onOpenRecent={(dir) => void openRecent(dir)}
        />
        {shortcuts && <RaccourcisView onClose={() => setShortcuts(false)} />}
        {signaler && (
          <SignalerView
            kind={signaler}
            album={null}
            index={-1}
            selected={null}
            onClose={() => setSignaler(null)}
          />
        )}
        {stockage && (
          <StockageView
            ouvertId={null}
            onSupprime={(id) => setRecents(forgetRecent(id))}
            onClose={() => setStockage(false)}
          />
        )}
      {maj && (
        <MajBandeau
          version={maj.version}
          enCours={majEnCours}
          onInstaller={() => {
            setMajEnCours(true);
            maj
              .install()
              .catch((e) => {
                setMajEnCours(false);
                setError(
                  fault(t("erreur.maj"), e),
                );
              });
          }}
          onPlusTard={() => setMaj(null)}
        />
      )}
        {apropos && <AProposView onClose={() => setApropos(false)} />}
        {prefs && <PrefsView onClose={() => setPrefs(false)} />}
      </>
    );
  }

  // The album emptied itself, one deletion at a time. Never a mute return
  // to the welcome screen: the way back is spelled out.
  if (total === 0) {
    return (
      <div className="empty">
        <div className="empty-block">
          <p className="kicker">Colophon</p>
          <div className="setup">
            <h1 className="setup-heading">L’album est vide</h1>
            <p className="lede">
              La dernière planche vient d’être supprimée. Rien n’est perdu :
              chaque suppression s’annule.
            </p>
            <p className="setup-actions">
              <button
                className="cta"
                autoFocus
                disabled={(hist?.past.length ?? 0) === 0}
                onClick={undo}
              >
                Ramener la dernière planche (⌘Z)
              </button>
              <button className="link" onClick={() => void closeAlbum()}>
                Composer un autre album
              </button>
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (bilan) {
    return (
      <BilanView
        bilan={bilan}
        album={album}
        curation={curation}
        variantes={variantes}
        choisie={variante}
        onChoisir={(id) => void basculerVariante(id)}
        onOpen={() => setBilan(null)}
        onTri={() => {
          // Straight into the keyboard review: the link promises a review,
          // not a grid to hunt through.
          setBilan(null);
          setView("tri");
          setRevue(0);
        }}
      />
    );
  }

  const onCover = index === COVER && view === "livre";
  const spread = onCover ? null : album.spreads[Math.min(index, total - 1)];
  /** Une planche de plus ou de moins, et l'application le dit. Le clavier de
   *  l'éditeur et la feuille de l'aperçu fidèle passent tous deux par ici :
   *  ce qui change la page ne doit exister qu'une fois. */
  const allerPlanche = (sens: number) => {
    const to = index + sens;
    if (to < 0 || to >= total) return false;
    setIndex(to);
    // Le clavier vient de traverser une page sans que rien ne le dise : la
    // ligne de statut est la seule voix de l'application, et elle est vivante
    // depuis peu.
    setStatus(t("planche.position", { n: to + 1, total }));
    return true;
  };
  const entries = triEntries(album, curation, opened?.thumb_srcs ?? []);
  const triEntry = entries.find((e) => e.src === triSelected) ?? null;

  return (
    <div className="app">
      <Bar
        album={album}
        dirty={dirty}
        canUndo={(hist?.past.length ?? 0) > 0}
        canRedo={(hist?.future.length ?? 0) > 0}
        view={view}
        triCount={entries.length}
        onView={gotoView}
        onUndo={undo}
        onRedo={redo}
        onSave={() => void save()}
        onRename={(titre) => {
          apply((a) => renameAlbum(a, titre));
          setStatus(t("etat.titre.modifie"));
        }}
        onRecompose={inTauri ? () => void recompose() : undefined}
        onFormat={inTauri ? () => void ouvrirBascule() : undefined}
        onOpen={inTauri ? undefined : openAlbum}
        onClose={inTauri ? undefined : closeAlbum}
      />
      {view === "tri" ? (
        <TriView
          entries={entries}
          selected={triSelected}
          onSelect={setTriSelected}
          onRescue={rescue}
          onRevue={() => {
            if (!entries.length) return;
            const at = triSelected
              ? entries.findIndex((x) => x.src === triSelected)
              : 0;
            setStatus(null);
            setRevue(Math.max(0, at));
          }}
        />
      ) : view === "envoi" ? (
        <EnvoiView
          album={album}
          printers={printers}
          profil={profil}
          onProfil={setProfil}
          onJump={(planche) => {
            setIndex(planche - 1);
            setSelected(null);
            setView("livre");
          }}
          onExport={() => void regenPdf()}
          exporting={rendering}
          exporte={exporte}
          dirty={dirty}
          colophonPossible={album.colophon !== undefined && album.colophon !== null}
          colophonActif={hasColophon(album)}
          onColophon={(on) => void toggleColophon(on)}
          gardeActif={hasGarde(album)}
          onGarde={(on) => void toggleGarde(on)}
          policeManquante={policeInfo?.manquante ?? false}
        />

      ) : view === "planches" ? (
        <PlanchesView
          album={album}
          current={index}
          onSelect={setIndex}
          onOpen={(at) => {
            setIndex(at);
            setView("livre");
          }}
          onMove={(from, to) => {
            apply((a) => moveSpread(a, from, to));
            setIndex(to);
            setStatus(t("planche.deplacee", { n: to + 1 }));
          }}
          onLock={(at) => apply((a) => toggleLock(a, at))}
        />
      ) : (
        <main className="stage">
          {/* La clé rejoue l'animation d'entrée à chaque planche — sauf dans
              l'aperçu fidèle, où le tour de feuille EST la transition : la
              remonter à chaque page tuerait le mouvement au milieu, et sa
              mémoire avec. */}
          <div className="turn" key={fidele ? "fidele" : index}>
            {fidele ? (
              <ApercuFidele
                onCover={onCover}
                page={index + 1}
                total={total}
                cle={pdfCle}
                album={album}
                onPlanche={allerPlanche}
                ref={feuilletage}
                onErreur={(m) =>
                  setError(fault(t("erreur.fidele"), m))
                }
              />
            ) : onCover ? (
              <CoverView
                album={album}
                printer={printers?.find((p) => p.id === profil) ?? null}
                onCover={(c) => apply((a) => setCover(a, c))}
                onReglage={(src, r) => apply((a) => setReglage(a, src, r))}
              />
            ) : (
              spread && (
                <SpreadView
                  album={album}
                  spread={spread}
                  planche={index}
                  selected={selected}
                  onSelect={setSelected}
                  onSwap={(a, b) => apply((al) => swapPhotos(al, index, a, b))}
                  onPlace={place}
                  onCrop={(slot, focal, zoom) =>
                    apply((a) => setSlotCrop(a, index, slot, focal, zoom))
                  }
                  onCaption={(slot, text) =>
                    apply((a) => setSlotCaption(a, index, slot, text))
                  }
                  onSpreadCaption={(c) =>
                    apply((a) => setSpreadCaption(a, index, c))
                  }
                  proposition={proposition}
                  onText={(text) => apply((a) => setSpreadText(a, index, text))}
                  onOverflow={setOverflow}
                  onSansMarge={() => setStatus(t("planche.recadrer.pleine.status"))}
                  onPlanche={allerPlanche}
                />
              )
            )}
          </div>
        </main>
      )}
      {view === "livre" && (
        <ContextLine
          album={album}
          spread={spread}
          onCover={onCover}
          selected={selected}
          onTemplate={(t) => apply((a) => changeTemplate(a, index, t))}
          onReglage={(src, r) => apply((a) => setReglage(a, src, r))}
          spreadIndex={index}
          fidele={fidele}
          onFidele={() => void basculerFidele()}
          onLock={
            spread
              ? () => {
                  const was = spread.locked;
                  apply((a) => toggleLock(a, index));
                  setStatus(
                    was
                      ? t("planche.liberee")
                      : t("planche.figee.status"),
                  );
                }
              : undefined
          }
        />
      )}
      {view === "livre" && !onCover && (
        <Drawer
          entries={entries}
          open={drawerOpen}
          onToggle={() => setDrawerOpen((o) => !o)}
        />
      )}
      {view === "tri" && revue !== null && entries.length > 0 && (
        <RevueView
          entries={entries}
          index={revue}
          status={status}
          onIndex={(i) => {
            if (i >= entries.length) {
              setRevue(null);
              setStatus("Revue terminée, chaque écart est vu");
            } else {
              setStatus(null);
              setRevue(i);
            }
          }}
          onRescue={rescue}
          onClose={() => setRevue(null)}
        />
      )}
      {view === "tri" ? (
        <TriFoot
          entry={triEntry}
          status={status}
          onRescue={rescue}
          onShowSpread={(src) => {
            const at = spreadOf(album, src);
            if (at < 0) return;
            setView("livre");
            setIndex(at);
            setTriSelected(null);
          }}
        />
      ) : view === "planches" ? (
        <PlanchesFoot
          album={album}
          index={Math.max(index, 0)}
          status={status}
          onInsert={(kind) => {
            const at = Math.max(index, 0);
            apply((a) => insertSpread(a, at, kind));
            setIndex(at + 1);
            setStatus(
              kind === "vide"
                ? t("planche.vide.inseree")
                : t("planche.texte.inseree"),
            );
          }}
          onDuplicate={() => {
            const at = Math.max(index, 0);
            apply((a) => duplicateSpread(a, at));
            setIndex(at + 1);
          }}
          onRemove={() => {
            const at = Math.max(index, 0);
            apply((a) => removeSpread(a, at));
            setStatus(t("planche.supprimee", { n: at + 1 }));
          }}
        />
      ) : view === "envoi" ? (
        <footer className="foot envoi-foot">
          <span className="status" role="status">{status}</span>
        </footer>
      ) : (
        <BookFoot
          album={album}
          index={index}
          total={total}
          status={status}
          overflow={overflow}
          rendering={rendering}
          onCancelExport={() => void cancelExport()}
          onSeek={(i) => setIndex(Math.min(total - 1, Math.max(COVER, i)))}
        />
      )}
      {opened && !opened.root_present && (
        <p className="warn">
          Dossier photo introuvable ({album.root}). L’aperçu tourne sur le cache
          de vignettes, l’export pleine résolution ne marchera pas.
        </p>
      )}
      {error && <FaultBlock fault={error} onDismiss={() => setError(null)} />}
      {shortcuts && <RaccourcisView onClose={() => setShortcuts(false)} />}
      {bascule && hist && (
        <BasculeView
          formats={basculeFormats}
          courant={hist.album.trim_mm}
          choisi={basculeChoisi}
          apercu={basculeApercu?.bilan ?? null}
          enCours={basculeEnCours}
          onChoisir={(f) => void apercuBascule(f)}
          onAppliquer={appliquerBascule}
          polices={polices}
          policeAlbum={hist.album.police ?? null}
          policeInfo={policeInfo}
          filtre={policeFiltre}
          onFiltre={setPoliceFiltre}
          onPolice={(p) => void choisirLaPolice(p)}
          onRendrePolice={rendreLaPolice}
          onClose={() => setBascule(false)}
        />

      )}
      {signaler && (
        <SignalerView
          kind={signaler}
          album={album}
          index={onCover ? -1 : Math.min(index, total - 1)}
          selected={selected}
          onClose={() => setSignaler(null)}
        />
      )}
      {stockage && (
        <StockageView
          ouvertId={opened?.dir ? albumId(opened.dir) : null}
          onSupprime={(id) => setRecents(forgetRecent(id))}
          onClose={() => setStockage(false)}
        />
      )}
      {maj && (
        <MajBandeau
          version={maj.version}
          enCours={majEnCours}
          onInstaller={() => {
            setMajEnCours(true);
            maj
              .install()
              .catch((e) => {
                setMajEnCours(false);
                setError(
                  fault(t("erreur.maj"), e),
                );
              });
          }}
          onPlusTard={() => setMaj(null)}
        />
      )}
      {apropos && <AProposView onClose={() => setApropos(false)} />}
        {prefs && <PrefsView onClose={() => setPrefs(false)} />}
    </div>
  );
}

/**
 * The album title, editable in place. It stayed frozen at the name of the
 * photo folder for seven sessions; a book whose title cannot be changed after
 * composition is a dead end, and the cover follows this field.
 *
 * Committed on Enter or on leaving the field, abandoned on Escape. An empty
 * name snaps back rather than leaving the book nameless: `renameAlbum`
 * refuses it, and the field must not show what the album does not carry.
 */

function TitreAlbum({
  titre,
  onRename,
}: {
  titre: string;
  onRename: (titre: string) => void;
}) {
  const [brouillon, setBrouillon] = useState(titre);
  const [edite, setEdite] = useState(false);
  // A recomposition, an undo or another album replaces the title under us.
  useEffect(() => {
    if (!edite) setBrouillon(titre);
  }, [titre, edite]);

  const valider = (el: HTMLInputElement) => {
    setEdite(false);
    // A field left with the caret at the end stays scrolled there, and a long
    // title would read from its middle for the rest of the session.
    el.scrollLeft = 0;
    if (brouillon.trim() === "" || brouillon.trim() === titre) {
      setBrouillon(titre);
      return;
    }
    onRename(brouillon);
  };

  return (
    <input
      className="bar-titre"
      value={brouillon}
      aria-label={t("bar.titre.aria")}
      spellCheck={false}
      // The half-title prints this line whole, on the narrowest format: the
      // engine measures that guarantee against this number
      // (`garde.rs::TITRE_MAX`), and the field is where it is held.
      maxLength={TITRE_MAX}
      onFocus={() => setEdite(true)}
      onChange={(e) => setBrouillon(e.target.value)}
      onBlur={(e) => valider(e.currentTarget)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          e.currentTarget.blur();
        } else if (e.key === "Escape") {
          e.preventDefault();
          setBrouillon(titre);
          setEdite(false);
          e.currentTarget.blur();
        }
      }}
    />
  );
}


/**
 * A newer version exists. A banner, not a dialog: nobody asked, and nothing
 * should stand between somebody and their album. Nothing has been downloaded
 * at this point; « Plus tard » dismisses it and the next launch asks again.
 */
function MajBandeau({
  version,
  enCours,
  onInstaller,
  onPlusTard,
}: {
  version: string;
  enCours: boolean;
  onInstaller: () => void;
  onPlusTard: () => void;
}) {
  return (
    <div className="maj" role="status">
      <span className="maj-texte">
        {t("maj.dispo", { version })}
        {enCours
          ? t("maj.encours")
          : t("maj.attente")}
      </span>
      <span className="maj-actions">
        <button className="cta small" onClick={onInstaller} disabled={enCours}>
          {enCours ? t("maj.installation") : t("maj.installer")}
        </button>
        <button className="link" onClick={onPlusTard} disabled={enCours}>
          {t("maj.plus.tard")}
        </button>
      </span>
    </div>
  );
}

/**
 * The bar tells three things apart, left to right: which album (the title),
 * where you are (the tabs, centred, nothing else among them), what you can
 * do to the file (a few quiet actions; everything else lives in the menu).
 * The context of the current spread is not the bar's business any more: it
 * sits on its own line next to the planche, in the book view.
 */
function Bar({
  album,
  dirty,
  canUndo,
  canRedo,
  view,
  triCount,
  onView,
  onUndo,
  onRedo,
  onSave,
  onRename,
  onRecompose,
  onFormat,
  onOpen,
  onClose,
}: {
  album: Album;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  view: View;
  triCount: number;
  onView: (v: View) => void;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
  onRename: (titre: string) => void;
  onRecompose?: () => void;
  onFormat?: () => void;
  /** Browser harness only: the shell reaches these through the menu. */
  onOpen?: () => void;
  onClose?: () => void;
}) {
  return (
    <header className="bar">
      <h1>
        <TitreAlbum titre={album.title} onRename={onRename} />
      </h1>
      <p className="meta">
        <span className="views" role="tablist">
          <button
            className={"view-tab" + (view === "livre" ? " active" : "")}
            onClick={() => onView("livre")}
            aria-keyshortcuts="Meta+1"
            title="⌘1"
          >
            {t("bar.livre")}
          </button>
          <button
            className={"view-tab" + (view === "tri" ? " active" : "")}
            onClick={() => onView("tri")}
            aria-keyshortcuts="Meta+2"
            title="⌘2"
          >
            {t("bar.tri")} · {triCount}
          </button>
          <button
            className={"view-tab" + (view === "planches" ? " active" : "")}
            onClick={() => onView("planches")}
            aria-keyshortcuts="Meta+3"
            title="⌘3"
          >
            {t("bar.planches")}
          </button>
          <button
            className={"view-tab" + (view === "envoi" ? " active" : "")}
            onClick={() => onView("envoi")}
            aria-keyshortcuts="Meta+4"
            title={t("bar.envoi.titre")}
          >
            {t("bar.envoi")}
          </button>
        </span>
      </p>
      <p className="actions">
        <button className="link" onClick={onUndo} disabled={!canUndo} title="⌘Z">
          {t("bar.annuler")}
        </button>
        <button className="link" onClick={onRedo} disabled={!canRedo} title="⇧⌘Z">
          {t("bar.retablir")}
        </button>
        <span className="actions-sep" aria-hidden="true" />
        {onRecompose && (
          <button
            className="link"
            onClick={onRecompose}
            title={t("bar.recomposer.titre")}
          >
            {t("bar.recomposer")}
          </button>
        )}
        {onFormat && (
          <button className="link" onClick={onFormat} title={t("bar.format.titre")}>
            {t("bar.format")}
          </button>
        )}
        <button
          className={"link" + (dirty ? " dirty" : "")}
          onClick={onSave}
          disabled={!dirty}
          title="⌘S"
        >
          {t("bar.enregistrer")}
        </button>
        {onClose && onOpen && (
          <>
            <span className="actions-sep" aria-hidden="true" />
            <button
              className="link"
              onClick={onClose}
              title={t("bar.nouveau.titre")}
            >
              {t("bar.nouveau")}
            </button>
            <button className="link" onClick={onOpen} title="⌘O">
              {t("bar.ouvrir")}
            </button>
          </>
        )}
      </p>
    </header>
  );
}

/**
 * The book view's context line, between the planche and its foot: which
 * template, edited badge, padlock, and the hint of the moment (crop gestures
 * on a selection, the drawer's whereabouts on the cover). Fixed height, so
 * the planche above never moves.
 */
function ContextLine({
  album,
  spread,
  onCover,
  selected,
  spreadIndex,
  onTemplate,
  onLock,
  onReglage,
  fidele,
  onFidele,
}: {
  album: Album;
  spread: Spread | null;
  onCover: boolean;
  selected: number | null;
  spreadIndex: number;
  onTemplate: (t: string) => void;
  onLock?: () => void;
  /** One history step through `edits.ts::setReglage`, at slider release. */
  onReglage: (src: string, reglage: Reglage) => void;
  /** The faithful preview is on, and the toggle that turns it off. */
  fidele: boolean;
  onFidele: () => void;
}) {
  // A spread that holds photographs, so a template picker means something:
  // the empty page, the text page and the two pages the machine writes
  // about the book have nothing to switch to.
  const photoSpread = spread ? templateCapacity(spread.template) > 0 : false;
  return (
    <div className="context-line">
      {/* The one control that leaves the DOM behind. Always here, on the
          cover as on a spread: the question « est-ce vraiment ça qui va
          s'imprimer ? » is asked of every page. */}
      <button
        className={"fidele-toggle" + (fidele ? " actif" : "")}
        onClick={onFidele}
        aria-pressed={fidele}
        title={
          fidele ? t("fidele.titre.off") : t("fidele.titre.on")
        }
      >
        {fidele ? t("fidele.actif") : t("fidele.voir")}
      </button>
      {onCover ? (
        <span className="context-hint">{t("contexte.couverture")}</span>
      ) : (
        spread && (
          <>
            {photoSpread && (
              <TemplatePicker
                album={album}
                spread={spread}
                index={spreadIndex}
                onPick={onTemplate}
              />
            )}
            <span className="spread-flags">
              {spread.edited && (
                <span
                  className="badge-edited"
                  title={t("table.editee")}
                />
              )}
              {onLock && (
                <button
                  className={"lock" + (spread.locked ? " locked" : "")}
                  onClick={onLock}
                  aria-pressed={spread.locked ?? false}
                  title={spread.locked ? t("table.figee") : t("table.figer")}
                >
                  <LockGlyph open={!spread.locked} />
                </button>
              )}
            </span>
            {/* The three adjustments of the chosen photo, native controls in
                a bar already tabbable: no sixth panel, no menu entry. */}
            {selected !== null && spread.slots[selected] && (
              <ReglageBloc
                src={spread.slots[selected].src}
                onCommit={onReglage}
              />
            )}
            <span className="context-hint">
              {selected !== null
                ? t("contexte.recadrage")
                : ""}
            </span>
          </>
        )
      )}
    </div>
  );
}

/**
 * The book's foot: page-turn arrows, a ruler graduated one tick per spread
 * (chapter starts marked in accent, their caption on hover), the cover
 * tick, the position, and a fixed line for statuses. The ruler navigates
 * and nothing else: reordering lives in the Planches view, its one home.
 * Constant height, whatever shows: the spread above never moves.
 */
function BookFoot({
  album,
  index,
  total,
  status,
  overflow,
  rendering,
  onCancelExport,
  onSeek,
}: {
  album: Album;
  index: number;
  total: number;
  status: string | null;
  overflow: string | null;
  rendering: boolean;
  onCancelExport: () => void;
  onSeek: (i: number) => void;
}) {
  return (
    <footer className="foot">
      <div className="foot-nav">
        <button
          className="foot-arrow"
          onClick={() => onSeek(index - 1)}
          disabled={index <= COVER}
          aria-label={t("nav.precedente")}
          title="←"
        >
          <Chevron dir="left" />
        </button>
        <button
          className={"ruler-cover" + (index === COVER ? " current" : "")}
          onClick={() => onSeek(COVER)}
          title={t("menu.couverture")}
          aria-label={t("menu.couverture")}
        >
          <CoverGlyph />
        </button>
        <nav className="ruler" aria-label={t("nav.aller")}>
          {album.spreads.map((s, i) => (
            <button
              key={i}
              className={
                "ruler-tick" +
                (s.caption ? " chapter" : "") +
                (i === index ? " current" : "")
              }
              style={{ left: `${total > 1 ? (i / (total - 1)) * 100 : 0}%` }}
              title={(s.caption ? `${s.caption} · ` : "") + t("nav.planche", { n: i + 1 })}
              onClick={() => onSeek(i)}
            />
          ))}
          {index >= 0 && (
            <span
              className="ruler-mark"
              style={{ left: `${total > 1 ? (index / (total - 1)) * 100 : 0}%` }}
            />
          )}
        </nav>
        <button
          className="foot-arrow"
          onClick={() => onSeek(index + 1)}
          disabled={index >= total - 1}
          aria-label={t("nav.suivante")}
          title={t("nav.espace.titre")}
        >
          <Chevron dir="right" />
        </button>
        <span className="foot-pos">
          {index === COVER ? "C" : index + 1} / {total}
        </span>
      </div>
      {/* La seule voix de l'application : « Photo à la taille exacte de sa
          case », « Planche déplacée en 12 ». Vivante, donc entendue par qui
          ne la voit pas — et posée sur le conteneur, qui lui existe avant
          le message, faute de quoi certains lecteurs d'écran ne verraient
          jamais naître la région. */}
      <div className="foot-line" role="status">
        {rendering ? (
          <span className="foot-render">
            {status ?? t("export.rendu")}{" "}
            <button className="link" onClick={onCancelExport}>
              {t("export.annuler")}
            </button>
          </span>
        ) : (
          <span className={overflow && !status ? "foot-overflow" : undefined}>
            {status ?? overflow ?? ""}
          </span>
        )}
      </div>
    </footer>
  );
}

/** The light table's foot: insertion and spread actions on the current one. */
function PlanchesFoot({
  album,
  index,
  status,
  onInsert,
  onDuplicate,
  onRemove,
}: {
  album: Album;
  index: number;
  status: string | null;
  onInsert: (kind: "vide" | "texte") => void;
  onDuplicate: () => void;
  onRemove: () => void;
}) {
  const spread = album.spreads[index];
  return (
    <footer className="foot">
      <div className="foot-nav planches-actions">
        <span className="foot-pos planches-pos">
          {t("planches.pos", { n: index + 1, total: album.spreads.length })}
          {spread?.caption ? ` · ${spread.caption}` : ""}
        </span>
        <button className="link" onClick={() => onInsert("vide")} title={t("planches.apres")}>
          {t("planches.inserer.vide")}
        </button>
        <button className="link" onClick={() => onInsert("texte")} title={t("planches.apres")}>
          {t("planches.inserer.texte")}
        </button>
        <span className="actions-sep" aria-hidden="true" />
        <button className="link" onClick={onDuplicate} title="⌘D">
          {t("menu.dupliquer")}
        </button>
        <button className="link" onClick={onRemove} title="⌫">
          {t("stockage.supprimer")}
        </button>
      </div>
      <p className="foot-line" role="status">
        {status ?? t("planches.hint")}
      </p>
    </footer>
  );
}

/**
 * The sorting view's foot. Nothing selected: what this view is. A photo
 * selected: its name, the photo that won over it (a click shows its spread),
 * and the rescue. Same fixed height as the book's foot.
 */
function TriFoot({
  entry,
  status,
  onRescue,
  onShowSpread,
}: {
  entry: TriEntry | null;
  status: string | null;
  onRescue: (entry: TriEntry) => void;
  onShowSpread: (src: string) => void;
}) {
  return (
    <footer className="foot">
      {entry ? (
        <div className="foot-tri">
          <code className="foot-tri-name">{entry.src.split("/").pop()}</code>
          {entry.kept && (
            <button
              className="foot-winner"
              onClick={() => onShowSpread(entry.kept!)}
              title={t("tri.gardee.voir")}
            >
              <MiniThumb src={entry.kept} />
              <span>{t("tri.gardee.label")}</span>
            </button>
          )}
          <button className="cta small" onClick={() => onRescue(entry)}>
            {t("revue.repecher")}
          </button>
        </div>
      ) : (
        <div className="foot-tri muted">{t("tri.foot.vide")}</div>
      )}
      <p className="foot-line" role="status">{status ?? ""}</p>
    </footer>
  );
}

/** A postage-stamp thumbnail, for the foot. */
function MiniThumb({ src }: { src: string }) {
  const [url, setUrl] = useState<string | undefined>(() => cachedThumb(src));
  useReglages();
  useEffect(() => {
    let alive = true;
    if (!cachedThumb(src)) setUrl(undefined);
    loadThumb(src).then(
      (u) => alive && setUrl(u),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [src]);
  return (
    <span className="mini-thumb">
      {url && <img src={url} alt="" style={{ filter: filtreDe(src) }} />}
    </span>
  );
}

/** The engine's format identifiers stay in the data; the screen reads the
 *  dictionaries (`format.*`), raw identifier when no entry exists. */
function formatLabel(name: string): string {
  const cle = `format.${name}`;
  return cle in FR ? t(cle as Cle) : name.replace(/-/g, " ");
}

function Empty({
  onOpen,
  onCreate,
  building,
  busyTitle,
  error,
  onDismissError,
  onCancelBuild,
  recents,
  onOpenRecent,
}: {
  onOpen: () => void;
  onCreate: (
    dir: string,
    format: string,
    spreads: number,
    densite: string,
    title: string | null,
  ) => void;
  building: string[] | null;
  busyTitle: string | null;
  error: Fault | null;
  onDismissError: () => void;
  onCancelBuild: () => void;
  recents: RecentAlbum[];
  onOpenRecent: (dir: string) => void;
}) {
  const [formats, setFormats] = useState<FormatPreset[]>([]);
  const [densities, setDensities] = useState<DensitePreset[]>([]);
  const [dir, setDir] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [format, setFormat] = useState("carre-21");
  const [spreads, setSpreads] = useState(48);
  const [densite, setDensite] = useState("equilibree");

  // Formats plus their geometry dumps: the large preview draws the real
  // spread grid, and every rectangle comes from the engine, never from here.
  const [geoPrets, setGeoPrets] = useState<Record<string, boolean>>({});
  useEffect(() => {
    listFormats().then((f) => {
      setFormats(f);
      f.forEach((preset) =>
        chargeGeometrieFormat(preset.w, preset.h, 0).then(
          () => setGeoPrets((g) => ({ ...g, [preset.name]: true })),
          () => {},
        ),
      );
    }, () => {});
    listDensities().then(setDensities, () => {});
  }, []);

  const folderName = dir?.split("/").pop() ?? "";
  const chosen = formats.find((f) => f.name === format);

  const pick = async () => {
    const picked = await pickPhotosFolder();
    if (!picked) return;
    setDir(picked);
    setTitle(picked.split("/").pop() ?? "");
  };

  return (
    <div className="empty">
      <div className={"empty-block" + (dir || building ? " wide" : "")}>
        <p className="kicker">Colophon</p>

        {!dir && !building && (
          <>
            <h1>
              {t("accueil.titre")
                .split("\n")
                .map((l, i) => (i === 0 ? l : [<br key={i} />, l]))}
            </h1>
            <p className="lede">{t("accueil.lede")}</p>
            <button className="cta" onClick={() => void pick()}>
              {t("accueil.choisir")}
            </button>
            <p className="hint">
              <button className="link" onClick={onOpen}>
                {t("accueil.ou.ouvrir")}
              </button>{" "}
              (<kbd>⌘</kbd> <kbd>O</kbd>)
            </p>
            {recents.length > 0 && (
              <div className="recents">
                <h2 className="recents-title">{t("accueil.recents")}</h2>
                <ul className="recents-list">
                  {recents.map((r) => (
                    <li key={r.dir}>
                      <button
                        className="recent"
                        onClick={() => onOpenRecent(r.dir)}
                        title={r.dir}
                      >
                        <span className="recent-nom">{r.title}</span>
                        <span className="recent-dir">{r.dir}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}

        {(dir || building) && (
          <div className="setup-layout">
            {dir && !building && (
              <form
                className="setup"
                onSubmit={(e) => {
                  e.preventDefault();
                  onCreate(dir, format, spreads, densite, title.trim() || null);
                }}
              >
                <h1 className="setup-heading">{t("setup.nouvel")}</h1>
                <p className="setup-folder">
                  <code>{dir}</code>
                  <button type="button" className="link" onClick={() => void pick()}>
                    {t("setup.changer.dossier")}
                  </button>
                </p>

                <div className="setup-duo">
                  <label className="setup-field">
                    <span className="setup-label">{t("setup.titre")}</span>
                    <input
                      className="setup-input"
                      value={title}
                      onChange={(e) => setTitle(e.target.value)}
                      placeholder={folderName}
                      autoFocus
                    />
                  </label>
                  <label className="setup-field">
                    <span className="setup-label">{t("setup.planches")}</span>
                    <span className="setup-spreads">
                      <input
                        className="setup-input narrow"
                        type="number"
                        min={8}
                        max={200}
                        value={spreads}
                        onChange={(e) => setSpreads(Number(e.target.value) || 48)}
                      />
                      <span className="setup-hint">
                        {t("setup.pages", { n: spreads * 2 })}
                      </span>
                    </span>
                  </label>
                </div>

                <div className="setup-field">
                  <span className="setup-label">{t("setup.format")}</span>
                  <div className="format-cards">
                    {formats.map((f) => (
                      <FormatCard
                        key={f.name}
                        f={f}
                        active={f.name === format}
                        onPick={() => setFormat(f.name)}
                      />
                    ))}
                  </div>
                </div>

                <div className="setup-field">
                  <span className="setup-label">{t("setup.rythme")}</span>
                  <div className="densites">
                    {densities.map((d) => (
                      <button
                        key={d.id}
                        type="button"
                        className={"densite" + (d.id === densite ? " active" : "")}
                        onClick={() => setDensite(d.id)}
                        aria-pressed={d.id === densite}
                      >
                        <span className="densite-nom">{d.nom}</span>
                        <span className="densite-about">{d.about}</span>
                        <DensiteApercu photos={d.photos_par_planche} />
                      </button>
                    ))}
                  </div>
                  <span className="setup-hint">{t("setup.rythme.hint")}</span>
                </div>

                <p className="setup-actions">
                  <button className="cta" type="submit">
                    {t("setup.composer")}
                  </button>
                  <button type="button" className="link" onClick={() => setDir(null)}>
                    {t("setup.annuler")}
                  </button>
                </p>
              </form>
            )}

            {building && (
              <div className="setup">
                <h1 className="setup-heading">
                  {busyTitle
                    ? t("compo.recomposition", { titre: busyTitle })
                    : t("compo.composition", {
                        titre: title.trim() || folderName || t("compo.album.defaut"),
                      })}
                </h1>
                <BuildProgress lines={building} onCancel={onCancelBuild} />
                <p className="setup-hint">
                  {busyTitle ? t("compo.hint.recomp") : t("compo.hint.compo")}
                </p>
              </div>
            )}

            {chosen && geoPrets[chosen.name] && <FormatSpreadPreview f={chosen} />}
          </div>
        )}

        {error && <FaultBlock fault={error} onDismiss={onDismissError} />}
      </div>
    </div>
  );
}

/** Three spreads' worth of cells at this pace, drawn small. Shows what the
 *  sentence says: how crowded a double page gets. */
function DensiteApercu({ photos }: { photos: number }) {
  // The average, rounded to the shapes a template actually lays out.
  const n = photos <= 2 ? 2 : photos <= 3.5 ? 4 : 6;
  const cols = n === 2 ? 1 : 2;
  return (
    <span className="densite-apercu" aria-hidden="true">
      {[0, 1].map((page) => (
        <span
          key={page}
          className="densite-page"
          style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}
        >
          {Array.from({ length: n / 2 }, (_, i) => (
            <span key={i} className="densite-case" />
          ))}
        </span>
      ))}
    </span>
  );
}

const cm = (mm: number) =>
  (mm / 10).toLocaleString(langue() === "fr" ? "fr-FR" : "en-GB", {
    maximumFractionDigits: 1,
  });

/**
 * One page format, drawn as an open double page at its true proportions:
 * the shapes differ, so the choice is visible before any vocabulary.
 */
function FormatCard({
  f,
  active,
  onPick,
}: {
  f: FormatPreset;
  active: boolean;
  onPick: () => void;
}) {
  const pageH = 32;
  const pageW = (f.w / f.h) * pageH;
  return (
    <button
      type="button"
      className={"format-card" + (active ? " active" : "")}
      onClick={onPick}
      title={f.about}
    >
      <span className="format-preview">
        <span className="format-page" style={{ width: pageW, height: pageH }} />
        <span className="format-page" style={{ width: pageW, height: pageH }} />
      </span>
      <span className="format-name">{formatLabel(f.name)}</span>
      <span className="format-dims">
        {cm(f.w)} × {cm(f.h)} cm
      </span>
    </button>
  );
}

/**
 * The chosen format, drawn large with the real spread geometry: the actual
 * margins, gutter and a six-photo template from the engine's own arithmetic,
 * plus its measurements. What you pick is what the press trims.
 */
function FormatSpreadPreview({ f }: { f: FormatPreset }) {
  const album = { trim_mm: { w: f.w, h: f.h }, bleed_mm: 0 } as Album;
  const geom = spreadGeometry(album);
  const rects = slotsFor("six", 6, geom);
  const width = 320;
  const scale = width / geom.w;

  return (
    <figure className="format-large">
      <span className="format-large-cote">{t("setup.cm.ouvert", { cm: cm(f.w * 2) })}</span>
      <div
        className="format-large-spread"
        style={{ width: geom.w * scale, height: geom.h * scale }}
      >
        {rects.map((r, i) => (
          <span
            key={i}
            className="format-large-slot"
            style={{
              left: r.x * scale,
              top: r.y * scale,
              width: r.w * scale,
              height: r.h * scale,
            }}
          />
        ))}
        <span className="format-large-fold" />
      </div>
      <figcaption>
        <strong>
          {cm(f.w)} × {cm(f.h)} cm
        </strong>{" "}
        {t("setup.la.page")} · {f.about}
      </figcaption>
    </figure>
  );
}

/**
 * The engine's progress lines, read into a bar and a stage label. The
 * analysis phase streams counts, so the bar moves all along the longest
 * stretch instead of freezing at « analyse ».
 */
function buildStage(lines: string[]): { pct: number; label: string } {
  let pct = 2;
  let label = t("stage.lecture");
  for (const l of lines) {
    let p = 0;
    let lab = "";
    const count = l.match(/^analyze: (\d+)\/(\d+)/);
    if (l.startsWith("scan:")) {
      p = 4;
      lab = t("stage.scan");
    } else if (count) {
      const [, i, n] = count;
      p = 5 + (65 * Number(i)) / Math.max(1, Number(n));
      lab = t("stage.analyse.n", { i, n });
    } else if (l.startsWith("analyze:")) {
      p = 70;
      lab = t("stage.analyse");
    } else if (l.startsWith("junk:") || l.startsWith("note:")) {
      p = 72;
      lab = t("stage.parasites");
    } else if (l.startsWith("dedup:")) {
      p = 76;
      lab = t("stage.dedup");
    } else if (l.startsWith("thinning:")) {
      p = 80;
      lab = t("stage.eclaircissage");
    } else if (l.startsWith("chapters:")) {
      p = 84;
      lab = t("stage.chapitres");
    } else if (l.startsWith("layout:")) {
      p = 88;
      lab = t("stage.layout");
    } else if (l.startsWith("pinned:")) {
      p = 90;
      lab = t("stage.pinned");
    } else if (l.startsWith("curation:")) {
      p = 92;
      lab = t("stage.curation");
    } else if (l.startsWith("pdf:")) {
      p = 96;
      lab = t("stage.pdf");
    }
    if (p >= pct) {
      pct = p;
      label = lab;
    }
  }
  return { pct, label };
}

function BuildProgress({
  lines,
  onCancel,
}: {
  lines: string[];
  onCancel: () => void;
}) {
  const { pct, label } = buildStage(lines);
  const log = lines.filter((l) => !/^analyze: \d+\/\d+$/.test(l)).slice(-6);
  return (
    <div className="build">
      <div
        className="build-bar"
        role="progressbar"
        aria-valuenow={Math.round(pct)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <span style={{ width: `${pct}%` }} />
      </div>
      <p className="build-stage">
        <span key={label} className="build-stage-label">
          {label}
        </span>
        <span className="build-actions">
          <button
            className="link"
            type="button"
            onClick={onCancel}
            title={t("compo.arreter.titre")}
          >
            {t("setup.annuler")}
          </button>
          <span className="build-pct">{Math.round(pct)} %</span>
        </span>
      </p>
      <details className="build-details">
        <summary>{t("compo.details")}</summary>
        <pre className="buildlog">
          {log.length ? log.join("\n") : t("compo.lecture")}
        </pre>
      </details>
    </div>
  );
}
