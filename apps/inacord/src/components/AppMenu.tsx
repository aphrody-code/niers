// Barre de menu de l'application (Fichier / Édition / Affichage), rendue DANS la barre supérieure
// custom — façon VS Code, Discord ou Spotify en mode sans bordure.
//
// Pourquoi pas `menu.setAsWindowMenu()` (ce que faisait `App.tsx` avant) : attacher un `HMENU` à
// la fenêtre demande à Windows de dessiner une barre de menu native, et une barre de menu native
// suppose une frame native — la fenêtre retrouvait donc sa bordure, sa légende et ses boutons
// système, annulant tout le `decorations: false` de `tauri.conf.json`. C'était la cause du
// « il y a toujours la bordure Win32 et les boutons close et le menu Win32 ».
//
// Ce qui est conservé : les menus déroulés restent de VRAIS menus popup Win32 (`Menu.popup()`,
// c'est-à-dire `TrackPopupMenu`, exactement comme le menu contextuel du clic droit), pas des
// `<div>` HTML. Seule la BARRE qui les héberge est à nous. Les accélérateurs clavier, eux, ne
// peuvent plus venir du menu de fenêtre : ils sont réenregistrés en écoute clavier globale
// (cf. `useAppMenuShortcuts`), donc ils fonctionnent toujours sans ouvrir le menu.
import { useEffect, useRef } from "react";
import { Menu, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { copySelectionActive, pasteActive, selectAllActive } from "@/lib/editBus";
import { getSettings, setSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";
import { IDS_VUES } from "@/lib/vues";

export interface AppMenuActions {
  onOpenExternalPath: (path: string) => void;
  onSelectTab: (tab: string) => void;
  tabLabels: Record<string, string>;
}

// L'ordre des vues vient du registre (`lib/vues.ts`), celui-là même que suit la barre latérale :
// cette liste était écrite à la main et « Cinéma » n'y figurait pas — la vue existait, se rendait,
// et n'était atteignable ni par le menu Affichage ni par un accélérateur.
//
// Au-delà de neuf entrées, l'accélérateur Ctrl+N n'existe plus (cf. `viewMenu`) : les vues
// suivantes restent accessibles par la barre latérale et le menu, sans raccourci. Le tableau de
// bord prend `Ctrl+1` en tant que page d'accueil, donc les neuf accélérateurs glissent d'un cran.
const VIEW_TABS = IDS_VUES;

function zoomIn() {
  setSettings({ uiZoom: Math.min(1.5, getSettings().uiZoom + 0.1) });
}
function zoomOut() {
  setSettings({ uiZoom: Math.max(0.7, getSettings().uiZoom - 0.1) });
}
function zoomReset() {
  setSettings({ uiZoom: 1 });
}

async function fileMenu(a: AppMenuActions): Promise<Menu> {
  return Menu.new({
    items: [
      {
        text: "Ouvrir un fichier…",
        action: async () => {
          const picked = await open({ title: "Ouvrir un fichier" });
          if (typeof picked === "string") a.onOpenExternalPath(picked);
        },
      },
      {
        text: "Ouvrir le dossier du jeu…",
        action: async () => {
          const picked = await open({ title: "Dossier du jeu", directory: true });
          if (typeof picked === "string") {
            setSettings({ gameDir: picked });
            toast.success(`Dossier du jeu : ${picked}`);
          }
        },
      },
      { text: "Ouvrir un CPK…", action: () => a.onSelectTab("cpk") },
      await PredefinedMenuItem.new({ item: "Separator" }),
      await PredefinedMenuItem.new({ text: "Quitter", item: "Quit" }),
    ],
  });
}

async function editMenu(): Promise<Menu> {
  // Ce ne sont PAS les `PredefinedMenuItem` Copy/SelectAll/Paste de l'OS : ces commandes texte
  // natives n'agissent que sur un champ de saisie focusé, jamais sur la liste de fichiers HTML
  // custom de l'Explorateur. Elles délèguent à la vue active via `editBus`.
  return Menu.new({
    items: [
      { text: "Copier le chemin\tCtrl+C", action: () => copySelectionActive() },
      { text: "Tout sélectionner\tCtrl+A", action: () => selectAllActive() },
      await PredefinedMenuItem.new({ item: "Separator" }),
      { text: "Coller…\tCtrl+V", action: () => pasteActive() },
    ],
  });
}

async function viewMenu(a: AppMenuActions): Promise<Menu> {
  return Menu.new({
    items: [
      ...VIEW_TABS.map((tab, i) => ({
        // Annoncer « Ctrl+10 » serait un mensonge : le raccourci n'est posé que pour les neuf
        // premières vues (`useAppMenuShortcuts` ne lit qu'un seul chiffre).
        text: i < 9 ? `${a.tabLabels[tab] ?? tab}\tCtrl+${i + 1}` : (a.tabLabels[tab] ?? tab),
        action: () => a.onSelectTab(tab),
      })),
      await PredefinedMenuItem.new({ item: "Separator" }),
      { text: "Zoom avant\tCtrl++", action: zoomIn },
      { text: "Zoom arrière\tCtrl+-", action: zoomOut },
      { text: "Réinitialiser le zoom\tCtrl+0", action: zoomReset },
    ],
  });
}

/**
 * Accélérateurs clavier de la barre de menu. Ils venaient du champ `accelerator` des items du menu
 * de fenêtre ; celui-ci ayant disparu (cf. en-tête), ils sont réenregistrés ici pour que Ctrl+1…8
 * et Ctrl+±/0 continuent de fonctionner sans ouvrir le moindre menu.
 *
 * Ctrl+C/Ctrl+A/Ctrl+V restent VOLONTAIREMENT hors de cette liste : la saisie de texte normale
 * (champ de recherche, éditeur Monaco, champ de renommage) doit garder le copier/coller du
 * navigateur. La vue active les expose déjà via `editBus` pour les listes de fichiers.
 */
export function useAppMenuShortcuts(a: AppMenuActions): void {
  const ref = useRef(a);
  ref.current = a;

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (!e.ctrlKey && !e.metaKey) return;
      const tag = (e.target as HTMLElement | null)?.tagName;
      const typing = tag === "INPUT" || tag === "TEXTAREA" || (e.target as HTMLElement | null)?.isContentEditable;

      if (e.key >= "1" && e.key <= "9") {
        const tab = VIEW_TABS[Number(e.key) - 1];
        if (tab) {
          e.preventDefault();
          ref.current.onSelectTab(tab);
        }
        return;
      }
      if (typing) return;
      if (e.key === "+" || e.key === "=") {
        e.preventDefault();
        zoomIn();
      } else if (e.key === "-") {
        e.preventDefault();
        zoomOut();
      } else if (e.key === "0") {
        e.preventDefault();
        zoomReset();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}

export function AppMenu({ actions, className }: { actions: AppMenuActions; className?: string }) {
  const ref = useRef(actions);
  ref.current = actions;

  async function popup(build: () => Promise<Menu>, el: HTMLElement) {
    try {
      const menu = await build();
      // Ancré sous le bouton : `popup()` sans position s'ouvrirait au curseur, ce qui ne
      // ressemblerait pas à une barre de menu.
      const rect = el.getBoundingClientRect();
      const { LogicalPosition } = await import("@tauri-apps/api/dpi");
      await menu.popup(new LogicalPosition(rect.left, rect.bottom));
    } catch (e) {
      toast.error(`Menu indisponible : ${e}`);
    }
  }

  const items: { label: string; build: () => Promise<Menu> }[] = [
    { label: "Fichier", build: () => fileMenu(ref.current) },
    { label: "Édition", build: () => editMenu() },
    { label: "Affichage", build: () => viewMenu(ref.current) },
  ];

  return (
    <div className={cn("flex items-center gap-0.5", className)}>
      {items.map((m) => (
        <button
          key={m.label}
          type="button"
          className="rounded-md px-2 py-1 text-xs text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
          onClick={(e) => void popup(m.build, e.currentTarget)}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
