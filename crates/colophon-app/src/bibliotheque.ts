// La photothèque du Mac, vue de l'écran.
//
// Le moteur n'en sait rien et n'en saura jamais rien : l'import produit un
// dossier de photographies, et l'écran de création reprend la main dessus
// comme sur n'importe quel dossier choisi au Finder. Ce fichier n'est donc
// pas une deuxième porte d'entrée, c'est un raccourci vers la seule.
//
// Le moteur de tout ce module est un fait mesuré le 02/09 : PhotoKit ne lève
// jamais pour un défaut d'accès, il rend une liste vide. Trois causes rendent
// cette même liste vide, et deux d'entre elles appellent une phrase que
// l'utilisateur peut agir. D'où `Etat`, qui n'a pas le droit d'être un
// booléen.

import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "./bridge";
import { Cle } from "./i18n";

export type AlbumPhotos = {
  id: string;
  nom: string;
  photos: number;
  /** Un album intelligent est un classement d'Apple, pas de l'utilisateur. */
  intelligent: boolean;
};

export type Etat =
  | { etat: "a-demander" }
  | { etat: "refuse" }
  | { etat: "injoignable"; chemin: string }
  | { etat: "lisible"; albums: AlbumPhotos[] }
  | { etat: "indisponible" };

export type RapportImport = {
  album: string;
  dossier: string;
  demandees: number;
  importees: number;
  octets: number;
  /** Nommées, pas seulement comptées. */
  absentes_du_mac: string[];
  echecs: { nom: string; motif: string }[];
};

/** L'état de la photothèque. Hors application, il n'y en a pas. */
export async function etatBibliotheque(): Promise<Etat> {
  if (!inTauri) return { etat: "indisponible" };
  return invoke<Etat>("photos_etat");
}

/** Pose la question à macOS, puis rend le nouvel état. */
export async function demanderAcces(): Promise<Etat> {
  if (!inTauri) return { etat: "indisponible" };
  return invoke<Etat>("photos_demander");
}

/** Le dossier d'import proposé : visible, dans les Images, nommé par l'album. */
export async function dossierPropose(nom: string): Promise<string> {
  if (!inTauri) return `~/Pictures/Colophon/${nom}`;
  return invoke<string>("photos_dossier_propose", { nom });
}

/** Importe un album dans un dossier. `reseau` reste faux au premier passage :
 *  ce qui n'est pas sur ce Mac se compte et se nomme, et rien ne part chercher
 *  des octets chez Apple avant que l'utilisateur ait vu le chiffre. */
export async function importerAlbum(
  album: string,
  dossier: string,
  reseau: boolean,
  onProgres?: (fait: number, total: number) => void,
): Promise<RapportImport> {
  if (!inTauri) throw new Error("la photothèque n’existe que dans l’application");
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<[number, number]>("photos:progres", (e) => {
    if (onProgres) onProgres(e.payload[0], e.payload[1]);
  });
  try {
    return await invoke<RapportImport>("photos_importer", { album, dossier, reseau });
  } finally {
    off();
  }
}

/** La clé i18n qui explique un état, ou `null` quand il n'y a rien à
 *  expliquer parce que la liste va s'afficher.
 *
 *  C'est ici que se joue le piège du module, et c'est ce que le test tient :
 *  une liste vide n'est pas une phrase, et « autorisé » n'est pas « lisible ».
 *  Le jour où quelqu'un remplace `Etat` par un booléen, ces quatre lignes
 *  deviennent impossibles à écrire, et c'est le but.
 *
 *  Le retour est typé sur les clés du dictionnaire, pas sur `string` : une
 *  clé absente d'i18n devient une erreur de compilation au lieu d'un libellé
 *  brut affiché à l'utilisateur. */
export function cleDeLetat(e: Etat): Cle | null {
  switch (e.etat) {
    case "a-demander":
      return "photos.demander";
    case "refuse":
      return "photos.refuse";
    case "injoignable":
      return "photos.injoignable";
    case "indisponible":
      return "photos.indisponible";
    case "lisible":
      return e.albums.length === 0 ? "photos.vide" : null;
  }
}

/** Un poids lisible, pour la phrase qui annonce un import. */
export function poids(octets: number): string {
  if (octets < 1024 * 1024) return `${Math.max(1, Math.round(octets / 1024))} Ko`;
  if (octets < 1024 * 1024 * 1024) return `${Math.round(octets / (1024 * 1024))} Mo`;
  return `${(octets / (1024 * 1024 * 1024)).toFixed(1)} Go`;
}
