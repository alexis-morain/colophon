// The native menu bar, built from the front: the bar is what makes the app's
// vocabulary visible (every shortcut reads here), and building it in TS lets
// the recent-albums list and the enabled states follow the React state
// without a Rust round trip. Windows will get the same structure through the
// same Tauri API.
//
// Menu accelerators and the window's own keydown handler can both see a
// chord depending on how WebKit routes key equivalents; App dedupes the two
// sources, so an action never runs twice for one keypress.

import { inTauri } from "./bridge";

export type RecentAlbum = { dir: string; title: string };

/** Everything the menu can ask of the app. Handlers no-op safely when the
 *  state does not allow them (no album, no current spread). */
export type MenuActions = {
  nouveau(): void;
  ouvrir(): void;
  ouvrirRecent(dir: string): void;
  enregistrer(): void;
  exporter(): void;
  fermerAlbum(): void;
  stockage(): void;
  annuler(): void;
  retablir(): void;
  vue(v: "livre" | "tri" | "planches" | "envoi"): void;
  couverture(): void;
  revue(): void;
  reserve(): void;
  gabarit(): void;
  dupliquer(): void;
  figer(): void;
  rendreAuto(): void;
  insererVide(): void;
  insererTexte(): void;
  supprimerPlanche(): void;
  raccourcis(): void;
  signalerBug(): void;
  signalerPlanche(): void;
  signalerRecadrage(): void;
};

/**
 * Build and install the application menu. Called again whenever the album
 * opens or closes, or the recents change: the menu is cheap to rebuild and
 * stale enabled-states are worse than the rebuild.
 */
export async function installMenu(
  get: () => MenuActions,
  albumOpen: boolean,
  recents: RecentAlbum[],
): Promise<void> {
  if (!inTauri) return;
  const { Menu, MenuItem, PredefinedMenuItem, Submenu } = await import(
    "@tauri-apps/api/menu"
  );

  const item = (
    id: keyof Omit<MenuActions, "ouvrirRecent" | "vue">,
    text: string,
    opts: { accelerator?: string; enabled?: boolean } = {},
  ) =>
    MenuItem.new({
      id,
      text,
      accelerator: opts.accelerator,
      enabled: opts.enabled ?? true,
      action: () => get()[id](),
    });

  const vue = (v: "livre" | "tri" | "planches" | "envoi", text: string, n: string) =>
    MenuItem.new({
      id: `vue-${v}`,
      text,
      accelerator: `CmdOrCtrl+${n}`,
      enabled: albumOpen,
      action: () => get().vue(v),
    });

  const sep = () => PredefinedMenuItem.new({ item: "Separator" });

  const appMenu = await Submenu.new({
    text: "Colophon",
    items: [
      await PredefinedMenuItem.new({ item: { About: null }, text: "À propos de Colophon" }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Hide", text: "Masquer Colophon" }),
      await PredefinedMenuItem.new({ item: "HideOthers", text: "Masquer les autres" }),
      await PredefinedMenuItem.new({ item: "ShowAll", text: "Tout afficher" }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Quit", text: "Quitter Colophon" }),
    ],
  });

  const recentItems =
    recents.length === 0
      ? [
          await MenuItem.new({
            id: "recents-vide",
            text: "Aucun album récent",
            enabled: false,
          }),
        ]
      : await Promise.all(
          recents.map((r, i) =>
            MenuItem.new({
              id: `recent-${i}`,
              text: r.title,
              action: () => get().ouvrirRecent(r.dir),
            }),
          ),
        );

  const fichier = await Submenu.new({
    text: "Fichier",
    items: [
      await item("nouveau", "Nouveau…", { accelerator: "CmdOrCtrl+N" }),
      await item("ouvrir", "Ouvrir…", { accelerator: "CmdOrCtrl+O" }),
      await Submenu.new({ text: "Albums récents", items: recentItems }),
      await sep(),
      await item("enregistrer", "Enregistrer", {
        accelerator: "CmdOrCtrl+S",
        enabled: albumOpen,
      }),
      await item("exporter", "Exporter…", {
        accelerator: "Shift+CmdOrCtrl+E",
        enabled: albumOpen,
      }),
      await sep(),
      // Storage answers a question about the machine, not about the album:
      // it opens with or without one, like the bug report does.
      await item("stockage", "Stockage…"),
      await sep(),
      await item("fermerAlbum", "Fermer l’album", { enabled: albumOpen }),
    ],
  });

  const edition = await Submenu.new({
    text: "Édition",
    items: [
      await item("annuler", "Annuler", {
        accelerator: "CmdOrCtrl+Z",
        enabled: albumOpen,
      }),
      await item("retablir", "Rétablir", {
        accelerator: "Shift+CmdOrCtrl+Z",
        enabled: albumOpen,
      }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Cut", text: "Couper" }),
      await PredefinedMenuItem.new({ item: "Copy", text: "Copier" }),
      await PredefinedMenuItem.new({ item: "Paste", text: "Coller" }),
      await PredefinedMenuItem.new({ item: "SelectAll", text: "Tout sélectionner" }),
    ],
  });

  const affichage = await Submenu.new({
    text: "Affichage",
    items: [
      await vue("livre", "Livre", "1"),
      await vue("tri", "Tri", "2"),
      await vue("planches", "Planches", "3"),
      await vue("envoi", "Envoi", "4"),
      await item("couverture", "Couverture", { enabled: albumOpen }),
      await sep(),
      await item("revue", "Passer en revue", { enabled: albumOpen }),
      await item("reserve", "Photos en réserve", { enabled: albumOpen }),
    ],
  });

  const planche = await Submenu.new({
    text: "Planche",
    items: [
      await item("gabarit", "Gabarit…", { enabled: albumOpen }),
      await item("dupliquer", "Dupliquer", {
        accelerator: "CmdOrCtrl+D",
        enabled: albumOpen,
      }),
      await item("figer", "Figer / libérer", {
        accelerator: "CmdOrCtrl+L",
        enabled: albumOpen,
      }),
      // The way out of the lock: sits right under it, where somebody who
      // just froze a spread by mistake will look for it.
      await item("rendreAuto", "Rendre à l’automatique…", { enabled: albumOpen }),
      await sep(),
      await item("insererVide", "Insérer une planche vide", { enabled: albumOpen }),
      await item("insererTexte", "Insérer une planche de texte", {
        enabled: albumOpen,
      }),
      await sep(),
      await item("supprimerPlanche", "Supprimer la planche", { enabled: albumOpen }),
    ],
  });

  const aide = await Submenu.new({
    text: "Aide",
    items: [
      await item("raccourcis", "Raccourcis clavier", {
        accelerator: "CmdOrCtrl+/",
      }),
      await sep(),
      // The three report variants mirror the repo's three issue templates.
      // A bug can be reported without an album; the two layout complaints
      // need one on screen.
      await item("signalerBug", "Signaler un problème…"),
      await item("signalerPlanche", "Signaler une planche ratée…", {
        enabled: albumOpen,
      }),
      await item("signalerRecadrage", "Signaler un recadrage raté…", {
        enabled: albumOpen,
      }),
    ],
  });

  const menu = await Menu.new({
    items: [appMenu, fichier, edition, affichage, planche, aide],
  });
  await menu.setAsAppMenu();
}
