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
import { t } from "./i18n";

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
  apropos(): void;
  preferences(): void;
  annuler(): void;
  retablir(): void;
  vue(v: "livre" | "tri" | "planches" | "envoi"): void;
  couverture(): void;
  apercuFidele(): void;
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
      // Ours, not the system panel: the system panel cannot carry the
      // GeoNames attribution, and that attribution is a licence condition.
      await item("apropos", t("menu.apropos")),
      await sep(),
      await item("preferences", t("menu.preferences"), {
        accelerator: "CmdOrCtrl+,",
      }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Hide", text: t("menu.masquer") }),
      await PredefinedMenuItem.new({ item: "HideOthers", text: t("menu.masquer.autres") }),
      await PredefinedMenuItem.new({ item: "ShowAll", text: t("menu.tout.afficher") }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Quit", text: t("menu.quitter") }),
    ],
  });

  const recentItems =
    recents.length === 0
      ? [
          await MenuItem.new({
            id: "recents-vide",
            text: t("menu.recents.vide"),
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
    text: t("menu.fichier"),
    items: [
      await item("nouveau", t("menu.nouveau"), { accelerator: "CmdOrCtrl+N" }),
      await item("ouvrir", t("menu.ouvrir"), { accelerator: "CmdOrCtrl+O" }),
      await Submenu.new({ text: t("menu.recents"), items: recentItems }),
      await sep(),
      await item("enregistrer", t("menu.enregistrer"), {
        accelerator: "CmdOrCtrl+S",
        enabled: albumOpen,
      }),
      await item("exporter", t("menu.exporter"), {
        accelerator: "Shift+CmdOrCtrl+E",
        enabled: albumOpen,
      }),
      await sep(),
      // Storage answers a question about the machine, not about the album:
      // it opens with or without one, like the bug report does.
      await item("stockage", t("menu.stockage")),
      await sep(),
      await item("fermerAlbum", t("menu.fermer.album"), { enabled: albumOpen }),
    ],
  });

  const edition = await Submenu.new({
    text: t("menu.edition"),
    items: [
      await item("annuler", t("menu.annuler"), {
        accelerator: "CmdOrCtrl+Z",
        enabled: albumOpen,
      }),
      await item("retablir", t("menu.retablir"), {
        accelerator: "Shift+CmdOrCtrl+Z",
        enabled: albumOpen,
      }),
      await sep(),
      await PredefinedMenuItem.new({ item: "Cut", text: t("menu.couper") }),
      await PredefinedMenuItem.new({ item: "Copy", text: t("menu.copier") }),
      await PredefinedMenuItem.new({ item: "Paste", text: t("menu.coller") }),
      await PredefinedMenuItem.new({ item: "SelectAll", text: t("menu.tout.selectionner") }),
    ],
  });

  const affichage = await Submenu.new({
    text: t("menu.affichage"),
    items: [
      await vue("livre", t("menu.livre"), "1"),
      await vue("tri", t("menu.tri"), "2"),
      await vue("planches", t("menu.planches"), "3"),
      await vue("envoi", t("menu.envoi"), "4"),
      await item("couverture", t("menu.couverture"), { enabled: albumOpen }),
      await sep(),
      // The one view that is not a second renderer: it reads the PDF the
      // press would read. Sits with the views, because that is what it is.
      await item("apercuFidele", t("menu.fidele"), {
        accelerator: "CmdOrCtrl+Shift+P",
        enabled: albumOpen,
      }),
      await sep(),
      await item("revue", t("menu.revue"), { enabled: albumOpen }),
      await item("reserve", t("menu.reserve"), { enabled: albumOpen }),
    ],
  });

  const planche = await Submenu.new({
    text: t("menu.planche"),
    items: [
      await item("gabarit", t("menu.gabarit"), { enabled: albumOpen }),
      await item("dupliquer", t("menu.dupliquer"), {
        accelerator: "CmdOrCtrl+D",
        enabled: albumOpen,
      }),
      await item("figer", t("menu.figer"), {
        accelerator: "CmdOrCtrl+L",
        enabled: albumOpen,
      }),
      // The way out of the lock: sits right under it, where somebody who
      // just froze a spread by mistake will look for it.
      await item("rendreAuto", t("menu.rendre.auto"), { enabled: albumOpen }),
      await sep(),
      await item("insererVide", t("menu.inserer.vide"), { enabled: albumOpen }),
      await item("insererTexte", t("menu.inserer.texte"), {
        enabled: albumOpen,
      }),
      await sep(),
      await item("supprimerPlanche", t("menu.supprimer.planche"), { enabled: albumOpen }),
    ],
  });

  const aide = await Submenu.new({
    text: t("menu.aide"),
    items: [
      await item("raccourcis", t("menu.raccourcis"), {
        accelerator: "CmdOrCtrl+/",
      }),
      await sep(),
      // The three report variants mirror the repo's three issue templates.
      // A bug can be reported without an album; the two layout complaints
      // need one on screen.
      await item("signalerBug", t("menu.signaler.bug")),
      await item("signalerPlanche", t("menu.signaler.planche"), {
        enabled: albumOpen,
      }),
      await item("signalerRecadrage", t("menu.signaler.recadrage"), {
        enabled: albumOpen,
      }),
    ],
  });

  const menu = await Menu.new({
    items: [appMenu, fichier, edition, affichage, planche, aide],
  });
  await menu.setAsAppMenu();
}
