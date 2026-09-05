// Barre supérieure — portage de `var/spacedrive/packages/interface/src/TopBar/TopBar.tsx` :
// positionnée en ABSOLU (`absolute top-0 z-[60] h-12`) au-dessus du contenu, décalée de la
// largeur de la barre latérale à gauche, et divisée en trois sections gauche/centre/droite
// (`TopBar/Section.tsx` amont). Toute la barre est une zone de déplacement de fenêtre
// (`data-tauri-drag-region`) : la fenêtre est `decorations: false`, cette barre EST la barre de
// titre.
//
// Ajout propre à la cible Windows : `WindowControls` à droite (spacedrive garde une barre de
// titre native et n'en a pas besoin).
import { Icon } from "@/components/ui/Icon";
import { WindowControls } from "@/components/ui/window-controls";

export function TopBar({
  sidebarWidth,
  title,
  onOpenPalette,
}: {
  sidebarWidth: number;
  title: string;
  onOpenPalette: () => void;
}) {
  return (
    <div
      className="absolute top-0 z-[60] h-12"
      data-tauri-drag-region
      style={{ left: sidebarWidth, right: 0 }}
    >
      <div
        className="relative flex h-full items-center gap-3 overflow-hidden px-3"
        data-tauri-drag-region
      >
        {/* Pas de barre Fichier/Édition/Affichage (demande utilisatrice) : les actions vivent
         * dans les menus contextuels natifs et les raccourcis clavier (`useAppMenuShortcuts`),
         * qui restent tous actifs. */}
        <span
          className="min-w-0 flex-1 truncate text-sm font-medium text-ink-dull"
          data-tauri-drag-region
          title={title}
        >
          {title}
        </span>

        <button
          type="button"
          onClick={onOpenPalette}
          className="flex shrink-0 items-center gap-1.5 rounded-md border border-app-line bg-app-box/60 px-2.5 py-1 text-xs text-ink-faint transition-colors hover:bg-app-hover hover:text-ink-dull"
        >
          <Icon name="search" size={13} />
          <span>Rechercher…</span>
          <kbd className="ml-1 rounded border border-app-line px-1 text-tiny opacity-70">Ctrl+K</kbd>
        </button>

        <WindowControls className="shrink-0" />
      </div>
    </div>
  );
}
