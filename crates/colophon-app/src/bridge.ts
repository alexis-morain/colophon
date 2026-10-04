// Single door to the backend. Inside the Tauri window it is the IPC; inside a
// plain browser it is the dev album server declared in vite.config.ts, which
// serves the exact same two things from a folder on disk. That fallback is how
// the book view gets checked without rebuilding the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { Ornement, setOrnements } from "./ornement";
import { Album, Discard, OpenedAlbum, Police, Spread } from "./album";

import { Dump, setGeometrie, setGeometrieFormat } from "./geometrie";
import { t } from "./i18n";
import type { Devis, Mode, Vue } from "./commande";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Native folder picker. In the browser, the dev server's folder is implied. */
export async function pickAlbumFolder(): Promise<string | null> {
  if (!inTauri) return "__dev__";
  return pickFolder("Choisir un dossier d’album");
}

/** Native folder picker for a folder of photos to compose from. In the
 *  browser the whole creation flow runs against the dev album, so any
 *  readable path does; it only feeds the title and the folder line. */
export async function pickPhotosFolder(): Promise<string | null> {
  if (!inTauri) return "~/Photos/corse-2013";
  return pickFolder("Choisir un dossier de photos");
}

async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title });
  return typeof picked === "string" ? picked : null;
}

export type FormatPreset = { name: string; w: number; h: number; about: string };

/** Page format presets, from the engine. A static mirror serves the browser
 *  harness so the creation screen can be worked on without the shell. */
export async function listFormats(): Promise<FormatPreset[]> {
  if (!inTauri) return DEV_FORMATS;
  return invoke<FormatPreset[]>("list_formats");
}

const DEV_FORMATS: FormatPreset[] = [
  { name: "carre-21", w: 210, h: 210, about: "carré 21 × 21, le format d’album courant" },
  { name: "carre-30", w: 300, h: 300, about: "carré 30 × 30, grand format de table" },
  { name: "portrait-a4", w: 210, h: 297, about: "A4 portrait" },
  { name: "paysage-a4", w: 297, h: 210, about: "A4 paysage" },
  { name: "paysage-28x21", w: 280, h: 210, about: "paysage 28 × 21" },
  { name: "portrait-20x25", w: 203, h: 254, about: "portrait 20 × 25, le 8 × 10 pouces" },
];

/** A composition pace offered at the first build, as the engine names it. */
export type DensitePreset = {
  id: string;
  nom: string;
  about: string;
  /** Photos per spread on average, for the small preview beside it. */
  photos_par_planche: number;
};

/** The composition paces, from the engine. Mirrored for the browser harness
 *  the way the formats are, so the creation screen works without the shell. */
export async function listDensities(): Promise<DensitePreset[]> {
  if (!inTauri) return DEV_DENSITES;
  return invoke<DensitePreset[]>("list_densities");
}

const DEV_DENSITES: DensitePreset[] = [
  {
    id: "aeree",
    nom: "Aérée",
    about:
      "Une ou deux photos par double page, souvent une seule en grand. Moins de photos retenues, chacune plus grande.",
    photos_par_planche: 2.1,
  },
  {
    id: "equilibree",
    nom: "Équilibrée",
    about:
      "Deux à quatre photos, avec des mosaïques de temps en temps. Le rythme par défaut.",
    photos_par_planche: 3.2,
  },
];

/** The three counts only the engine knows at the end of a build; the
 *  discard detail comes from curation.json. Shown once, before the book. */
export type BuildBilan = {
  photos_scanned: number;
  photos_kept: number;
  chapters: number;
};

/** One proposal composed beside the album, as the creation screen shows it. */
export type VarianteResume = {
  /** Handle: `album.<id>.json` on disk. Empty for the one asked for. */
  id: string;
  nom: string;
  about: string;
  planches: number;
  photos: number;
  /** Three photo sources spread across the book, for the thumbnails. */
  apercu: string[];
};

export type BuiltAlbum = {
  opened: OpenedAlbum;
  bilan: BuildBilan;
  variantes: VarianteResume[];
};

/** Swap in one of the proposals composed beside the album. Reversible until
 *  the first save, which takes the unchosen ones off the disk. */
export async function chooseVariante(id: string): Promise<OpenedAlbum> {
  if (!inTauri) {
    throw new Error(
      "Les propositions vivent dans le dossier de l’album, que le serveur de dev ne sert qu’une fois.",
    );
  }
  const opened = await invoke<OpenedAlbum>("choose_variante", { id });
  await chargeGeometrie();
  return opened;
}

/** Build an album from a photo folder, then open it. Long: seconds cold. */
export async function buildAlbum(
  photosDir: string,
  format: string,
  spreads: number,
  densite: string,
  title: string | null,
): Promise<BuiltAlbum> {
  if (!inTauri) {
    const built = await devBuild();
    await chargeGeometrie();
    return built;
  }
  const built = await invoke<BuiltAlbum>("build_album_from_folder", {
    photosDir,
    format,
    spreads,
    densite,
    title,
  });
  await chargeGeometrie();
  return built;
}

/** The album already composed from this photo folder, if any: its folder,
 *  for the creation screen to offer reopening it rather than composing a
 *  second one beside it. Null in the browser harness, which has one album. */
export async function albumExistant(photosDir: string): Promise<string | null> {
  if (!inTauri) return null;
  return invoke<string | null>("album_existant", { photosDir });
}

/** Subscribe to the engine's progress lines. Returns the unsubscribe. */
export async function onBuildProgress(
  cb: (line: string) => void,
): Promise<() => void> {
  if (!inTauri) {
    devProgressListeners.add(cb);
    return () => devProgressListeners.delete(cb);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("build:progress", (e) => cb(e.payload));
}

/* The browser harness stands in for the engine: the same progress lines the
 * Rust side emits, paced so the whole creation flow can be watched and
 * styled without the shell, then the dev album opens as the result. */
const devProgressListeners = new Set<(line: string) => void>();

async function devBuild(): Promise<BuiltAlbum> {
  const emit = (line: string) => devProgressListeners.forEach((cb) => cb(line));
  const tick = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const photos = 575;
  emit(`scan: ${photos} photos`);
  await tick(300);
  for (let i = 20; i <= photos; i += 20) {
    emit(`analyze: ${i}/${photos}`);
    await tick(60);
  }
  emit(`analyze: ${photos} photos, 4.1s`);
  for (const line of [
    "junk: 3 écartées",
    "dedup: 96 doublons",
    "thinning: 95 écartées",
    "chapters: 9 chapitres",
    "layout: 48 planches",
    "curation: 419 entrées",
    "pdf: album.pdf",
  ]) {
    await tick(280);
    emit(line);
  }
  await tick(200);
  const opened = await openAlbum("__dev__");
  // Three proposals from the harness's one album: the counts and the wording
  // are what the screen has to lay out, the photos are the same.
  const avecPhoto = opened.album.spreads.filter((s) => s.slots.length > 0);
  const apercu = [1, 2, 3]
    .map((q) => avecPhoto[Math.floor((avecPhoto.length * q) / 4)])
    .filter(Boolean)
    .map((s) => s.slots[0].src);
  return {
    opened,
    bilan: { photos_scanned: photos, photos_kept: 152, chapters: 9 },
    variantes: [
      {
        id: "autre-rythme",
        nom: "Aérée",
        about:
          "Une ou deux photos par double page, souvent une seule en grand. Moins de photos retenues, chacune plus grande.",
        planches: opened.album.spreads.length,
        photos: 101,
        apercu,
      },
      {
        id: "resserree",
        nom: "Plus court",
        about:
          "Un tiers de planches en moins, donc moins de photos retenues. Un livre qui se feuillette d’un trait, et qui coûte moins cher à imprimer.",
        planches: Math.round(opened.album.spreads.length * 0.68),
        photos: 68,
        apercu,
      },
    ],
  };
}

export async function openAlbum(path: string): Promise<OpenedAlbum> {
  if (inTauri) {
    const opened = await invoke<OpenedAlbum>("open_album", { path });
    await chargeGeometrie();
    return opened;
  }
  const res = await fetch("/__dev/album");
  if (!res.ok) throw new Error(await res.text());
  if (!res.headers.get("content-type")?.includes("json")) {
    throw new Error(
      "Serveur de dev sans album : relancez avec COLOPHON_ALBUM=<dossier> npm run dev",
    );
  }
  const opened = (await res.json()) as OpenedAlbum;
  await chargeGeometrie();
  return opened;
}

/**
 * Charger le pack d'ornements, une fois pour la vie de la fenêtre.
 *
 * **Il ne bloque rien.** La géométrie est refusée quand elle manque, parce
 * qu'un album sans elle ne se dessine pas ; un pack absent laisse un album
 * entier lisible, avec des ornements que personne ne peut poser. Un moteur
 * qui rendrait ici serait un moteur qu'on vient de casser, et l'écran ne doit
 * pas mourir avec lui.
 */
export async function chargeOrnements(): Promise<void> {
  try {
    if (inTauri) {
      setOrnements(await invoke<Ornement[]>("ornements"));
      return;
    }
    const res = await fetch("/__dev/ornements");
    if (res.ok) setOrnements((await res.json()) as Ornement[]);
  } catch {
    // Rien : le pack reste vide, et le panneau des ornements le dira.
  }
}

/**
 * Load the engine's geometry dump for the album that just opened, before it
 * reaches React: every rectangle the editor draws comes from this dump, so
 * an album without it must not render at all.
 */
async function chargeGeometrie(): Promise<void> {
  if (inTauri) {
    setGeometrie(await invoke<Dump>("geometrie"));
    return;
  }
  const res = await fetch("/__dev/geometrie");
  if (!res.ok) throw new Error(await res.text());
  setGeometrie(await res.json());
}

/** The dump for a bare page format (creation screen previews). */
export async function chargeGeometrieFormat(
  w: number,
  h: number,
  bleed: number,
): Promise<void> {
  if (inTauri) {
    setGeometrieFormat(await invoke<Dump>("geometrie_format", { w, h, bleed }));
    return;
  }
  const res = await fetch(
    `/__dev/geometrie?format=${w}x${h}&bleed=${bleed}`,
  );
  if (!res.ok) throw new Error(await res.text());
  setGeometrieFormat(await res.json());
}

/** One face this machine carries, as the picker shows it. `rang` is a rank
 *  in the list that was just returned and means nothing outside it: the
 *  front end never holds a font's path, the way it never holds a
 *  thumbnail's. */
export type PoliceOfferte = {
  rang: number;
  famille: string;
  nom: string;
  postscript: string;
  /** Engine code (`illisible`, `embarquement_interdit`, `bitmap_seulement`,
   *  `cmap_illisible`, `format_non_embarquable`), worded by `i18n.ts`. */
  refus: string | null;
  /** Ce que disent les tables de la face (OS/2, `post`, `head`), jamais son nom. */
  gras: boolean;
  italique: boolean;
};

/** Every face installed on this machine, refused ones included: a picker
 *  that hides what it refuses sends people hunting for a missing font. */
export async function polices_installees(): Promise<PoliceOfferte[]> {
  if (!inTauri) return [];
  return invoke<PoliceOfferte[]>("polices_installees");
}

/** Copy the chosen face into the album's folder. The record comes back and
 *  goes into the album through the edit history, so ⌘Z undoes the choice. */
export async function choisirPolice(rang: number): Promise<Police> {
  return invoke<Police>("choisir_police", { rang });
}

/** Les faces d'une famille, avec leur rang dans la liste que tient le
 *  moteur : là où les boutons gras et italique d'un bloc cherchent la face
 *  qui le dit. Vide hors de l'application, comme la liste elle-même. */
export async function policesDeFamille(famille: string): Promise<PoliceOfferte[]> {
  if (!inTauri) return [];
  return invoke<PoliceOfferte[]>("polices_de_famille", { famille });
}

/** Copier une face à côté de l'album pour un bloc. La fiche revient et se
 *  pose sur l'objet par l'historique d'édition, donc ⌘Z l'annule. */
export async function poserPoliceObjet(rang: number): Promise<Police> {
  return invoke<Police>("poser_police_objet", { rang });
}

/** Les octets de la face d'un bloc, tels que l'émetteur les embarque.
 *  Rejette avec `fichier_absent` quand le fichier est parti ou que son nom
 *  sort de la grammaire : le bloc se dessine alors dans la face du livre,
 *  comme au PDF. */
export async function policeObjetOctets(fichier: string): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("police_objet_octets", { fichier });
  const res = await fetch(`/__dev/police-objet?fichier=${encodeURIComponent(fichier)}`);
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** What the album will actually be set in, resolved exactly as the emitter
 *  resolves it. `manquante` is the one thing the screen must not swallow. */
export type PoliceEtat = { manquante: boolean; postscript: string; octets: number };

export async function policeEtat(fichier?: string): Promise<PoliceEtat> {
  if (!inTauri) {
    return { manquante: false, postscript: "SourceSans3-Regular", octets: 431196 };
  }
  return invoke<PoliceEtat>("police_etat", { fichier: fichier ?? null });
}

/** The bytes of the face the emitter will embed. Measured and drawn with on
 *  this side, so screen and paper cannot answer differently. */
export async function policeOctets(fichier?: string): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("police_octets", { fichier: fichier ?? null });
  const res = await fetch(
    `/__dev/police${fichier ? `?fichier=${encodeURIComponent(fichier)}` : ""}`,
  );
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** The bytes of a face the picker is *offering*, so its name can be drawn
 *  in it. Same extraction as choosing it, nothing written beside the album:
 *  a specimen is what the PDF would embed, not what the disk holds.
 *
 *  `max` is a ceiling in bytes; the engine refuses a heavier face rather
 *  than shipping it over the bridge, and the caller then simply leaves that
 *  name in the interface's own face. Outside the shell there is no list of
 *  installed faces at all, so there is nothing to draw. */
export async function policeApercu(rang: number, max: number): Promise<ArrayBuffer> {
  if (!inTauri) throw new Error("aperçu de police indisponible hors de l’application");
  return invoke<ArrayBuffer>("police_apercu", { rang, max });
}

export async function fetchThumb(src: string): Promise<ArrayBuffer> {

  if (inTauri) return invoke<ArrayBuffer>("thumb", { src });
  const res = await fetch(`/__dev/thumb?src=${encodeURIComponent(src)}`);
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** A blocking yes/no question. window.confirm silently returns true inside
 *  the Tauri webview (no JS dialogs in WKWebView), which would turn every
 *  guard into a rubber stamp: the native dialog plugin asks for real. */
export async function confirmDialog(message: string): Promise<boolean> {
  if (!inTauri) return window.confirm(message);
  const { ask } = await import("@tauri-apps/plugin-dialog");
  // Les libellés, parce que le greffon met « Yes » et « No » en dur : la
  // question était en français et les deux boutons en anglais, sur les sept
  // dialogues de l'application. Mesuré à l'écran le 21/09.
  return ask(message, {
    title: "Colophon",
    kind: "warning",
    okLabel: t("commun.oui"),
    cancelLabel: t("commun.non"),
  });
}

/** Quitter l'application, pour de bon. Le menu Quitter est le nôtre depuis
 *  que le travail non enregistré doit être demandé avant d'être jeté : un
 *  `PredefinedMenuItem` passe par `NSApplication.terminate`, qui ne déroule
 *  rien et ne demande rien.
 *
 *  Deux chemins, parce qu'une app qu'on ne peut plus quitter serait pire que
 *  le défaut corrigé : `exit` du greffon processus, et si la permission
 *  manquait, la destruction de la fenêtre. Dans le navigateur de
 *  développement il n'y a pas de processus à fermer. */
export async function quitApp(): Promise<void> {
  if (!inTauri) {
    window.close();
    return;
  }
  try {
    const { exit } = await import("@tauri-apps/plugin-process");
    await exit(0);
  } catch {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().destroy();
  }
}

/** La pastille rouge demande avant de détruire la fenêtre. `garder` retient
 *  la fermeture ; ne rien appeler la laisse aller. Rend de quoi se
 *  désabonner. Hors Tauri il n'y a pas de fenêtre native à écouter, et
 *  `beforeunload` tient ce rôle. */
export async function surFermeture(
  demande: (garder: () => void) => Promise<void> | void,
): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().onCloseRequested(async (e) => {
    await demande(() => e.preventDefault());
  });
}

/** Recompose the open album from its photo folder. Edited and locked
 *  spreads survive verbatim; progress streams like a build. Tauri only. */
export async function recomposeAlbum(): Promise<OpenedAlbum> {
  if (!inTauri) {
    throw new Error("recomposition hors application : utilisez la commande colophon");
  }
  return invoke<OpenedAlbum>("recompose_album");
}

/** Abandon the composition in flight. The engine stops between photos. */
/** What a bascule would do to the open album, without doing it. */
export type BasculeBilan = {
  trim_avant: { w: number; h: number };
  trim_apres: { w: number; h: number };
  planches: number;
  planches_inchangees: number;
  sous_resolution: { planche: number; src: string; ppi_avant: number; ppi_apres: number }[];
  deja_sous_resolution: number;
  replis: { planche: number; avant: string; apres: string }[];
  inaptes: { planche: number; gabarit: string; trahison: number }[];
  epinglees_touchees: number[];
  tailles_manquantes: string[];
  couverture_sous_resolution:
    | { planche: number; src: string; ppi_avant: number; ppi_apres: number }
    | null;
};

/** The same album in another format, proposed. Nothing is written: the caller
 *  applies the album through the edit history, so ⌘Z undoes it and ⌘S commits
 *  it, exactly like every other edit. */
export async function basculeAlbum(
  w: number,
  h: number,
  profil: string,
): Promise<{ album: Album; bilan: BasculeBilan }> {
  return invoke<{ album: Album; bilan: BasculeBilan }>("bascule_album", { w, h, profil });
}

export async function cancelBuild(): Promise<void> {
  if (!inTauri) return;
  return invoke("cancel_build");
}

/** Abandon the print render in flight. No half-written PDF can survive. */
export async function cancelExport(): Promise<void> {
  if (!inTauri) return;
  return invoke("cancel_export");
}

/** EXIF date of a photo, formatted for a caption suggestion, or null. The
 *  dev server answers from the same table as « Dater » (`/__dev/dates`), so
 *  the popover's buttons show in a browser too. */
export async function captionSuggestion(src: string): Promise<string | null> {
  if (inTauri) return invoke<string | null>("caption_suggestion", { src });
  const res = await fetch("/__dev/dates");
  if (!res.ok) return null;
  const dates: Record<string, string> = await res.json();
  return dates[src] ?? null;
}

/** La date fiable de chaque photo posée, par src, écrite comme la
 *  proposition de la case (`legende::date_de`). Une photo absente de la
 *  table n'a pas de date fiable. Le serveur de dev lit l'album du disque. */
export async function datesFiables(album: Album): Promise<Record<string, string>> {
  if (inTauri) return invoke<Record<string, string>>("dates_fiables", { album });
  const res = await fetch("/__dev/dates");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** The caption proposed for a spread whose caption is empty: town when it
 *  diverges from the chapter, day when the chapter covers several. Null is
 *  silence, and silence is a full answer. */
export async function legendeProposee(planche: number): Promise<string | null> {
  if (inTauri) return invoke<string | null>("proposition_legende", { planche });
  const res = await fetch(`/__dev/proposition?planche=${planche + 1}`);
  if (!res.ok) return null;
  return res.json();
}

/** Templates the spread can switch to right now, count and orientation
 *  both fitting: the engine's one rule (`gabarit::compatibles`), photos
 *  passed live so an unsaved edit filters right. Each name comes with its
 *  betrayal — how far the worst photo is from its cell — which is what
 *  lets `gabarit.ts` offer one entry per arrangement and apply, inside it,
 *  the cell shape these photos fit best. Null when the engine cannot answer
 *  (no server, unreadable thumbs): the caller then filters nothing rather
 *  than guessing. */
export async function gabaritsCompatibles(
  srcs: string[],
): Promise<[string, number][] | null> {
  try {
    if (inTauri) {
      return await invoke<[string, number][]>("gabarits_compatibles", { srcs });
    }
    const res = await fetch(
      `/__dev/gabarits?srcs=${encodeURIComponent(JSON.stringify(srcs))}`,
    );
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

/** Face-anchored focal point, recomputed on the thumbnail. The crop
 *  editor's double-click recentres on it. */
export async function detectedFocal(src: string): Promise<[number, number]> {
  if (!inTauri) return [0.5, 0.42];
  return invoke<[number, number]>("detected_focal", { src });
}

/** The photos curation set aside. Empty for albums built before the export. */
export async function fetchCuration(): Promise<Discard[]> {
  if (inTauri) return invoke<Discard[]>("curation");
  const res = await fetch("/__dev/curation");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** Pourquoi une photo de la réserve est à sa place, en code : le libellé est
 *  à `i18n.ts`. Miroir de `reserve::Raison`. */
export type Raison = "meme_jour" | "proche" | "nette";

/** Une photo de la réserve, telle que le moteur la classe. `focal` est celui
 *  qu'elle gardera une fois posée : la case se construit sans relire la
 *  curation. */
export type Candidat = {
  src: string;
  score: number;
  taken?: string;
  raison: Raison;
  focal: [number, number];
};

/** La réserve classée pour une planche, et ce qui a manqué pour la classer
 *  tout à fait (`note`). */
export type Reserve = { candidats: Candidat[]; note?: string };

/** Les photos qu'aucune planche ne montre, classées pour la planche `at` :
 *  le même jour d'abord, puis par écart de date et par score, sans les
 *  quasi-doublons de ce qu'elle porte, douze au plus. L'album voyage pour
 *  qu'une photo retirée à l'instant soit déjà de la réserve. Le serveur de
 *  dev, lui, lit `album.json` : le harnais enregistre avant de demander.
 *  Sans serveur de dev, une liste vide. */
export async function reserveClassee(album: Album, at: number): Promise<Reserve> {
  if (inTauri) return invoke<Reserve>("reserve_classee", { album, planche: at });
  try {
    const res = await fetch(`/__dev/reserve?planche=${at + 1}`);
    if (!res.ok) return { candidats: [] };
    return await res.json();
  } catch {
    return { candidats: [] };
  }
}

/** La fiche d'une photo, telle que le moteur la lit (`fiche::FichePhoto`) :
 *  le fichier, la prise de vue, le lieu, les mesures du relevé s'il y en a.
 *  Jamais un chemin : `nom` est le nom du fichier. */
export type FichePhoto = {
  nom: string;
  largeur: number;
  hauteur: number;
  octets: number;
  format: string;
  taken: string;
  taken_reliable: boolean;
  appareil: string | null;
  objectif: string | null;
  ouverture: number | null;
  temps_de_pose: string | null;
  iso: number | null;
  focale_mm: number | null;
  gps: [number, number] | null;
  lieu: { nom: string; pays: string } | null;
  note: number | null;
  nettete: number | null;
  exposition: number | null;
};

/** La fiche d'une photo par son `src` ; le moteur refuse tout `src` qui
 *  n'est pas un nom de fichier, et forme le chemin depuis `album.root`. */
export async function photoFiche(src: string): Promise<FichePhoto> {
  if (inTauri) return invoke<FichePhoto>("photo_fiche", { src });
  const res = await fetch(`/__dev/fiche?src=${encodeURIComponent(src)}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** Ce que l'alerte de qualité lit d'une photo (`qualite::Mesure`) : la
 *  taille de l'original, orientation appliquée, et deux mesures de
 *  l'analyse. */
export type MesurePhoto = {
  largeur: number;
  hauteur: number;
  nettete: number;
  exposition: number;
};

/** Le relevé de l'album ouvert, par `src`, et le seuil de flou du dossier
 *  (le dernier décile ; `null` pour un petit dossier). */
export type ReleveAlbum = {
  photos: Record<string, MesurePhoto>;
  seuil_flou: number | null;
};

/** Le relevé, lu une fois par album : du `releve.json` de l'album quand il
 *  en porte un, des vignettes sinon. Le serveur de dev lance le même
 *  moteur. */
export async function releveAlbum(): Promise<ReleveAlbum> {
  if (inTauri) return invoke<ReleveAlbum>("releve_album");
  const res = await fetch("/__dev/releve");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** Montrer l'original dans le gestionnaire de fichiers, sélectionné. Le
 *  chemin se résout et se vérifie côté moteur (canonique, sous `album.root`).
 *  Dans le navigateur il n'y a pas de Finder : rien ne se passe. */
export async function revelerPhoto(src: string): Promise<void> {
  if (!inTauri) return;
  return invoke("reveler_photo", { src });
}

/** Overwrite album.json, atomically on both sides of the bridge. */
/** The characters of this album its face cannot draw, each once. The engine
 *  prints them as `?`, and the screen must show the same. Empty in the
 *  browser harness, whose face is the engine's own. */
export async function policeAbsents(album: Album): Promise<string[]> {
  if (!inTauri) return [];
  return invoke<string[]>("police_absents", { album });
}

export async function saveAlbum(album: Album): Promise<void> {
  if (inTauri) return invoke("save_album", { album });
  const res = await fetch("/__dev/album", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(album),
  });
  if (!res.ok) throw new Error(await res.text());
}

/** A printer profile, exactly as the engine holds it. Never restated here:
 *  a supplier's specs live in `printer.rs` and travel across the bridge. */
export type Printer = {
  id: string;
  nom: string;
  pdf_x: "x4" | "aucun";
  espace: "rgb" | "fogra39";
  bleed_mm: { haut: number; bas: number; exterieur: number; dos: number };
  /** Case-wrap cotes: turn-in, board overhang, hinge groove. Zero for every
   *  supplier who does not wrap boards, and the sheet is then the flat one. */
  rempli_mm: number;
  debord_mm: number;
  mors_mm: number;
  safe_mm: number;
  fichiers: "un" | "deux";
  /** The supplier reads one PDF page as one book page. The album is composed
   *  in spreads either way: the export cuts each one in two on its way out. */
  pages_simples: boolean;
  dos: { mode: "fourni" } | { mode: "calcule"; mm_par_feuille: number; constante_mm: number; certitude: Certitude };
  pages_min: number;
  pages_max: number;
  pas_pagination: number;
  min_ppi: number;
  certitude: Certitude;
  reserves: string[];
};

export type Certitude = "confirme" | "provisoire";

/** One thing wrong with the file, named the way a human would name it. */
export type Defaut = {
  regle: string;
  bloquant: boolean;
  /** 1-based, as the ruler shows it. */
  planche?: number;
  case?: number;
  src?: string;
  cause: string;
  remede: string;
};

/** The sheet handed to whoever receives the PDF. */
export type Fiche = {
  imprimeur: string;
  format_page_mm: [number, number];
  planches: number;
  pages_interieur: number;
  /** Pages in the delivered PDF, cover leaves included when there is one
   *  file: the count to declare at the order. */
  pages_fichier: number;
  fond_perdu_mm: { haut: number; bas: number; exterieur: number; dos: number };
  zone_sure_mm: number;
  /** The flat cover sheet, when the supplier expects one from us. Absent when
   *  they bind a single file and build their own cover. */
  feuille_couverture_mm?: [number, number];
  espace: "rgb" | "fogra39";
  output_intent: string;
  conformite: "x4" | "aucun";
  fichiers: "un" | "deux";
  dos_mm?: number;
  grammage_g_m2: number;
  resolution_cible_dpi: number;
};

export type PrevolReport = {
  album: string;
  profil: string;
  ok: boolean;
  bloquants: number;
  avertissements: number;
  fiche: Fiche;
  reserves?: string[];
  notes?: string[];
  defauts: Defaut[];
};

/** The printer profiles the engine knows. Outside the shell they come from
 *  the dev album server, which runs the same engine: no second copy of a
 *  supplier's specs anywhere in the front end. */
export async function listPrinters(): Promise<Printer[]> {
  if (inTauri) return invoke<Printer[]>("list_printers");
  const res = await fetch("/__dev/printers");
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

/** One audit counter, counts only. The engine's report also carries the
 *  finding details, which may name photos: the report panel never quotes
 *  them, the numbers alone travel. */
export type AuditCounter = {
  count: number;
  /** Absent = le compteur avertit et ne décide pas. Miroir de
   *  `audit.rs::Counter::seuil`, dont le `None` est un régime et pas un
   *  oubli : il n'y a rien à dépasser, donc rien à relever pour faire taire
   *  la règle. */
  seuil?: number;
  dur: boolean;
};

export type AuditSummary = {
  ok: boolean;
  planches: number;
  compteurs: Record<string, AuditCounter>;
  notes?: string[];
};

/** The raw material of a problem report, gathered on this machine and shown
 *  in full before anything is sent anywhere. */
export type ReportData = {
  version: string;
  os: string;
  /** Last log lines, paths already reduced to file names at write time. */
  log: string;
  /** Null without an album or when the audit fails: the report says so. */
  audit: AuditSummary | null;
};

export async function reportData(): Promise<ReportData> {
  if (inTauri) return invoke<ReportData>("report_data");
  const compteur = (count: number, seuil: number, dur: boolean) => ({
    count,
    seuil,
    dur,
  });
  const avertit = (count: number) => ({ count, dur: false });
  return {
    version: "dev",
    os: "harnais navigateur",
    log: [
      "2026-08-16 10:02:11 démarrage, version dev",
      "2026-08-16 10:02:40 scan: 575 photos",
      "2026-08-16 10:02:44 layout: 48 planches",
      "2026-08-16 10:03:02 export 300 dpi, profil cloudprinter",
      "2026-08-16 10:05:19 export terminé",
    ].join("\n"),
    audit: {
      ok: true,
      planches: 48,
      compteurs: {
        visage_coupe: compteur(0, 0, true),
        orientation_trahie: compteur(0, 0, true),
        doublon_planche: compteur(0, 0, true),
        sous_resolution: compteur(1, 3, false),
        chapitre_orphelin: compteur(0, 0, false),
        ouverture_faible: compteur(0, 2, false),
        rythme_plat: compteur(0, 1, false),
        legende_manquante: compteur(2, 4, false),
        legende_sur_photo: compteur(0, 0, true),
        repetition_gabarit: compteur(0, 0, true),
        objet_hors_marge: avertit(0),
        objet_deborde: avertit(0),
        ornement_sur_photo: avertit(0),
      },
    },
  };
}

/** Open the pre-filled issue form. In the shell a guarded Rust command hands
 *  the URL to the system browser; the harness opens a tab. */
export async function openReportUrl(url: string): Promise<void> {
  if (!inTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  return invoke("open_report_url", { url });
}

/**
 * Ask the release feed whether a newer version exists. Returns its version
 * number, or null when there is nothing (and when there is no network, and
 * when the feed cannot be read): an app that cannot reach GitHub is an app
 * that works, and saying so out loud at every launch would be noise.
 *
 * Nothing is downloaded here. The download and the restart are a deliberate
 * click, in the notice this returns.
 */
export async function checkUpdate(): Promise<{
  version: string;
  notes: string;
  install: () => Promise<void>;
} | null> {
  if (!inTauri) return null;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const maj = await check();
    if (!maj) return null;
    return {
      version: maj.version,
      notes: maj.body ?? "",
      install: async () => {
        await maj.downloadAndInstall();
        const { relaunch } = await import("@tauri-apps/plugin-process");
        await relaunch();
      },
    };
  } catch {
    // Offline, feed unreachable, signature refused: all the same to the
    // user, who did not ask. The next launch will try again.
    return null;
  }
}

/** What the About screen shows: the version, and the third-party notices
 *  generated from the two lock files and embedded in the binary. */
export type AboutData = { version: string; notices: string };

export async function aboutData(): Promise<AboutData> {
  if (!inTauri) {
    return {
      version: "dev",
      notices:
        "Les notices sont générées à la compilation (scripts/notices.sh) et " +
        "embarquées dans le binaire : le harnais navigateur n’en a pas.",
    };
  }
  return invoke<AboutData>("about_data");
}

/** Re-render album.pdf, the preview file, from the saved album.json. Seconds
 *  on a fifty-spread album: it draws from the thumbnail cache. */
export async function renderPdf(): Promise<string> {
  if (!inTauri) {
    throw new Error("Le rendu du PDF se fait dans l’application, pas au navigateur.");
  }
  return invoke<string>("render_pdf");
}

/** Raw bytes of one of the album's own PDFs, for the faithful preview. The
 *  two names are a closed set on the Rust side: no path travels here. */
export async function albumPdfBytes(
  quoi: "album" | "couverture",
): Promise<ArrayBuffer> {
  if (inTauri) return invoke<ArrayBuffer>("album_pdf_bytes", { quoi });
  const res = await fetch(`/__dev/pdf?quoi=${quoi}`);
  if (!res.ok) throw new Error(await res.text());
  return res.arrayBuffer();
}

/** Render the flat cover sheet into the album folder, for its preview. Same
 *  renderer the export uses, same profile. */
export async function renderCoverPreview(profil: string): Promise<string> {
  if (!inTauri) {
    throw new Error("La couverture se rend dans l’application, pas au navigateur.");
  }
  return invoke<string>("render_cover_preview", { profil });
}

/** The colophon page, rendered from the facts the album carries. Null on an
 *  album composed before the page existed: nothing can be invented after the
 *  fact, so the Envoi screen simply does not offer it. */
export async function colophonSpread(album: Album): Promise<Spread | null> {
  if (!inTauri) {
    // The harness has no engine; the shape is enough to work the screen.
    if (!album.colophon) return null;
    return {
      template: "colophon",
      slots: [],
      text:
        "Colophon\n\n152 photographies retenues sur 575, prises du 21 au 29 octobre 2013.\n" +
        "Porto-Vecchio et Bonifacio.\nCanon EOS 550D.\n\n" +
        "Composé le 17 août 2026 avec Colophon dev.\n210 × 210 mm, papier 150 g/m².",
    };
  }
  return invoke<Spread | null>("colophon_spread", { album });
}

/** The half-title page, rendered from the facts the album carries and from
 *  the title it carries right now. Null on an album composed before the
 *  facts existed: nothing can be invented after the fact, so the Envoi
 *  screen simply does not offer it. */
export async function gardeSpread(album: Album): Promise<Spread | null> {
  if (!inTauri) {
    // The harness has no engine; the shape is enough to work the screen.
    if (!album.colophon) return null;
    return {
      template: "garde",
      slots: [],
      text: `${album.title}\n\nDu 21 au 29 octobre 2013\nPorto-Vecchio, Bonifacio`,
    };
  }
  return invoke<Spread | null>("garde_spread", { album });
}

/** The composer's own version of one spread, for « rendre à l'automatique ».
 *  Null when the spread was inserted by hand: nothing automatic proposed it.
 *  Throws when the album predates album.origin.json, and the message says so.
 *  Nothing is written: the caller applies it through the undo stack. */
export async function originSpread(
  album: Album,
  index: number,
): Promise<Spread | null> {
  if (!inTauri) {
    throw new Error(
      "La version automatique vit dans album.origin.json, que le serveur de dev ne sert pas.",
    );
  }
  return invoke<Spread | null>("origin_spread", { album, index });
}

/** One album folder as the storage panel shows it. The three weights are
 *  separated because they are not equally expensive to lose. */
export type AlbumEntry = {
  id: string;
  title: string;
  /** Page format in millimetres. Null when album.json could not be read. */
  format: [number, number] | null;
  spreads: number | null;
  /** Seconds since the epoch. */
  modified: number | null;
  bytes_total: number;
  bytes_thumbs: number;
  bytes_pdf: number;
  /** Set when album.json is unreadable: the row still shows and can be
   *  deleted, which is exactly what such an album is good for. */
  probleme: string | null;
};

export type StorageReport = {
  dir: string;
  total: number;
  albums: AlbumEntry[];
};

/** What the app has written on this disk. Walks the data directory, so it
 *  runs off the main thread on the Rust side. */
export async function listAlbums(): Promise<StorageReport> {
  if (!inTauri) return DEV_STORAGE;
  return invoke<StorageReport>("list_albums");
}

const DEV_STORAGE: StorageReport = {
  dir: "~/Library/Application Support/fr.morain.colophon",
  total: 702 * 1024 * 1024,
  albums: [
    {
      id: "random-2024-55846e90",
      title: "random-2024",
      format: [210, 210],
      spreads: 44,
      modified: 1786539600,
      bytes_total: 200 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 46 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "corse-2013-88f933b1",
      title: "Corse 2013",
      format: [210, 210],
      spreads: 48,
      modified: 1786366800,
      bytes_total: 183 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 44 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "mauritanie-2019-9ed43672",
      title: "mauritanie-2019",
      format: [297, 210],
      spreads: 30,
      modified: 1786107600,
      bytes_total: 136 * 1024 * 1024,
      bytes_thumbs: 104 * 1024 * 1024,
      bytes_pdf: 31 * 1024 * 1024,
      probleme: null,
    },
    {
      id: "froid-2013-ea70098b",
      title: "froid-2013",
      format: null,
      spreads: null,
      modified: null,
      bytes_total: 183 * 1024 * 1024,
      bytes_thumbs: 138 * 1024 * 1024,
      bytes_pdf: 44 * 1024 * 1024,
      probleme: "album.json illisible : EOF while parsing a value",
    },
  ],
};

/** Delete one album folder and return the bytes freed. The photos it was
 *  composed from are never touched: the Rust side cannot reach them. */
export async function deleteAlbum(id: string): Promise<number> {
  if (!inTauri) {
    const i = DEV_STORAGE.albums.findIndex((a) => a.id === id);
    if (i < 0) return 0;
    const [gone] = DEV_STORAGE.albums.splice(i, 1);
    DEV_STORAGE.total -= gone.bytes_total;
    return gone.bytes_total;
  }
  return invoke<number>("delete_album", { id });
}

/** Empty every thumbnail cache, returning the bytes freed. The caches
 *  rebuild themselves at the next open, they are the only thing that does. */
export async function purgeThumbCaches(): Promise<number> {
  if (!inTauri) {
    let freed = 0;
    for (const a of DEV_STORAGE.albums) {
      freed += a.bytes_thumbs;
      a.bytes_total -= a.bytes_thumbs;
      a.bytes_thumbs = 0;
    }
    DEV_STORAGE.total -= freed;
    return freed;
  }
  return invoke<number>("purge_thumb_caches");
}

/** Show the data directory in the system file manager. */
export async function revealDataDir(): Promise<void> {
  if (!inTauri) return;
  return invoke("reveal_data_dir");
}

/** Preflight the saved album against one profile. Seconds on a big album:
 *  it reopens every original to measure the effective resolution. */
export async function preflight(profil: string): Promise<PrevolReport> {
  if (inTauri) return invoke<PrevolReport>("preflight", { profil });
  const res = await fetch(`/__dev/prevol?profil=${encodeURIComponent(profil)}`);
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(text);
  }
}

/** Ce que « Préparer » a écrit : le nom du dossier (jamais son chemin), les
 *  fichiers posés, et le prévol relu sur ces fichiers-là. */
export type Preparation = {
  dossier: string;
  fichiers: string[];
  rapport: PrevolReport;
};

/** Prépare le dossier de la commande : la boîte native choisit un dossier,
 *  le moteur y écrit l'intérieur, la couverture, `export.json` et
 *  `fiche.txt`, puis relit le prévol sur eux. Null quand la boîte est
 *  fermée. Le chemin reste côté moteur, comme pour `exportPdf`.
 *
 *  Au harnais, le serveur de dev appelle `colophon --preparer` dans le
 *  dossier jetable que `COLOPHON_PREPARER` nomme (le répertoire temporaire
 *  du système à défaut), jamais à côté de l'album. */
export async function preparerDossier(
  title: string,
  profil: string,
  onProgress?: (done: number, total: number) => void,
): Promise<Preparation | null> {
  if (!inTauri) {
    const res = await fetch(`/__dev/preparer?profil=${encodeURIComponent(profil)}`, {
      method: "POST",
    });
    const text = await res.text();
    if (!res.ok) throw new Error(text);
    return JSON.parse(text);
  }
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<string>("export:progress", (e) => {
    const m = /^render: (\d+)\/(\d+)/.exec(e.payload);
    if (m && onProgress) onProgress(Number(m[1]), Number(m[2]));
  });
  try {
    return await invoke<Preparation | null>("preparer_dossier", { titre: title, profil });
  } finally {
    off();
  }
}

/** Rouvre le dernier dossier préparé dans le Finder. Le moteur tient le
 *  chemin ; au harnais il n'y a pas de Finder, rien ne se passe. */
export async function montrerDossierPrepare(): Promise<void> {
  if (!inTauri) return;
  return invoke("montrer_dossier_prepare");
}

/** Ask where to keep the PDF (Téléchargements by default), then render it
 *  at print resolution straight to that path. The dialog comes first: the
 *  render reopens every original at 300 dpi and takes minutes, nobody
 *  should wait through it before being asked a question. Progress arrives
 *  as (done, total) photo counts. Returns the chosen path, or null when
 *  the dialog is dismissed. Tauri only: the dev server has no engine. */
export async function exportPdf(
  title: string,
  profil: string,
  onProgress?: (done: number, total: number) => void,
): Promise<string[] | null> {
  if (!inTauri) {
    throw new Error("PDF hors application : utilisez la commande colophon");
  }
  // The destination is asked by the engine side, never sent from here: a
  // command that wrote at any path the webview named was a primitive this
  // window had no business holding. An empty answer is a cancelled dialog.
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<string>("export:progress", (e) => {
    const m = /^render: (\d+)\/(\d+)/.exec(e.payload);
    if (m && onProgress) onProgress(Number(m[1]), Number(m[2]));
  });
  try {
    const ecrits = await invoke<string[]>("export_pdf", { titre: title, profil });
    return ecrits.length === 0 ? null : ecrits;
  } finally {
    off();
  }
}

// ---- la commande (K-s3) ------------------------------------------------------
//
// Toutes les commandes passent par le moteur, qui tient la clé et les accès
// dans `commande.json` : la fenêtre ne reçoit jamais qu'une `Vue`. Au
// harnais, `/__dev/commande/*` rejoue un faux CloudCore en mémoire (les
// réponses du faux HTTP de K-s2) : aucune clé, aucun réseau.

export type OptionProduit = {
  reference: string;
  type_: string;
  note: string | null;
  defaut: boolean;
};

export type Pays = { code: string; nom: string; etat_requis: boolean };

/** Ce que l'écran Commander propose pour le dossier préparé. */
export type OffreCommande = {
  format: string;
  /** Null : ce format n'a pas de produit, l'écran le dit en une ligne. */
  produit: string | null;
  pages: number;
  /** Le papier intérieur, figé sur celui du dossier préparé : la
   *  couverture porte son dos. Null quand le format n'a pas de produit. */
  papier: OptionProduit | null;
  finitions: OptionProduit[];
  couverture: string | null;
  pays: Pays[];
};

export type Cote = { ok: boolean; produits: number | null; erreur: string | null };
export type Verification = { cloudprinter: Cote | null; depot: Cote | null };

export type AdresseSaisie = {
  prenom: string;
  nom: string;
  rue: string;
  rue2: string | null;
  code_postal: string;
  ville: string;
  email: string;
  telephone: string;
};

export type Relu = { vue: Vue; erreur: string | null };

async function commandeDev<T>(action: string, corps: object = {}): Promise<T> {
  const res = await fetch(`/__dev/commande/${action}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(corps),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(text);
  return JSON.parse(text);
}

function commandeAppel<T>(commande: string, action: string, args: Record<string, unknown> = {}): Promise<T> {
  return inTauri ? invoke<T>(commande, args) : commandeDev<T>(action, args);
}

export const commandeVue = () => commandeAppel<Vue>("commande_vue", "vue");

export const commandeCle = (cle: string, mode: Mode) =>
  commandeAppel<Vue>("commande_cle", "cle", { cle, mode });

export const commandeCleRetirer = () => commandeAppel<Vue>("commande_cle_retirer", "cle_retirer");

export const commandeDepot = (d: {
  endpoint: string;
  region: string;
  bucket: string;
  identifiant: string;
  secret: string;
}) => commandeAppel<Vue>("commande_depot", "depot", d);

export const commandeDepotRetirer = () =>
  commandeAppel<Vue>("commande_depot_retirer", "depot_retirer");

export const commandeVerifier = () => commandeAppel<Verification>("commande_verifier", "verifier");

export const commandeOffre = () => commandeAppel<OffreCommande>("commande_offre", "offre");

export const commandeDevis = (c: {
  couverture: string;
  finition: string;
  quantite: number;
  pays: string;
}) => commandeAppel<Devis>("commande_devis", "devis", c);

/** Passe la commande sur le devis tenu par le moteur. La progression arrive
 *  par étape (`interieur`, `couverture`, `commande`). */
export async function commandePasser(
  quote: string,
  adresse: AdresseSaisie,
  onEtape?: (etape: string) => void,
): Promise<Vue> {
  if (!inTauri) return commandeDev<Vue>("passer", { quote, adresse });
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<string>("commande:progress", (e) => onEtape?.(e.payload));
  try {
    return await invoke<Vue>("commande_passer", { quote, adresse });
  } finally {
    off();
  }
}

export const commandeRelire = (reference?: string) =>
  commandeAppel<Relu>("commande_relire", "relire", { reference: reference ?? null });

export const commandeAnnuler = (reference: string) =>
  commandeAppel<Relu>("commande_annuler", "annuler", { reference });
