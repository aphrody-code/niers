// Barre latérale de navigation — portage du layout de
// `var/spacedrive/packages/interface/src/components/SpacesSidebar/index.tsx` :
// panneau FLOTTANT de 220 px (`p-2` autour, `rounded-2xl`, `bg-sidebar/65`), `nav` intérieure en
// `p-2.5` avec `pt-[43px]` pour laisser passer la zone de titre, contenu défilant masqué en bas
// (`mask-fade-out`), et une rangée de `CircleButton` épinglée en pied.
//
// Comme l'amont, elle porte À LA FOIS la navigation principale et les emplacements : spacedrive
// empile `SpaceItem` (vues), `LocationsGroup`, `VolumesGroup` et `TagsGroup` dans UNE seule barre.
// nie-explorer avait deux barres latérales concurrentes (les onglets en haut + un `PlacesSidebar`
// interne à l'Explorateur) — elles sont fusionnées ici, comme l'amont.
//
// Le rendu d'un item reprend `SpaceItem.tsx` : `rounded-md`, icône 16 px, libellé tronqué,
// actif = `bg-accent`.
import { useTheme } from "next-themes";

import { CircleButton } from "@/components/ui/circle-button";
import { Icon } from "@/components/ui/Icon";
import { JobManagerButton } from "@/components/JobManager";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export interface SidebarSection {
  /** Intitulé du groupe (`GroupHeader.tsx` amont) — `null` pour un groupe sans titre. */
  label: string | null;
  items: SidebarItem[];
}

export interface SidebarItem {
  id: string;
  label: string;
  icon: string;
  /** Info-bulle (défaut : `label`). */
  title?: string;
  /** Surlignage actif — si absent, comparé à `current`. */
  active?: boolean;
  /** Action au clic — si absente, `onSelect(id)`. */
  onClick?: () => void;
  /** Menu contextuel natif (clic droit), cf. `lib/contextMenu.ts`. */
  onContextMenu?: (e: React.MouseEvent) => void;
  /** Clic milieu — React ne le remonte PAS dans `onClick` : sans ce gestionnaire dédié, le geste
   * « ouvrir dans un nouvel onglet » serait inatteignable à la souris. */
  onAuxClick?: (e: React.MouseEvent) => void;
  /** Couleur de l'icône (classe Tailwind) — ex. épingles en `text-accent`. */
  iconClassName?: string;
}

export function Sidebar({
  sections,
  current,
  onSelect,
  onOpenSettings,
  onBasculerRepli,
}: {
  sections: SidebarSection[];
  current: string;
  onSelect: (id: string) => void;
  onOpenSettings: () => void;
  /** Absent = pas de bouton de repli (la barre reste telle quelle). */
  onBasculerRepli?: () => void;
}) {
  const t = useT();
  const { resolvedTheme, setTheme } = useTheme();
  const dark = resolvedTheme !== "light";

  return (
    // Panneau à plat : plus de carte interne (`rounded-2xl bg-sidebar/65`) ni de double marge —
    // la barre laisse voir le fond de la fenêtre et ne s'en détache que par sa teinte.
    <div className="flex h-full w-[200px] min-w-[168px] max-w-[280px] flex-col bg-transparent">
      <div className="flex h-full flex-col overflow-hidden bg-sidebar/30">
        {/* Pas d'en-tête de marque : ni logo ni titre « niers » (demande utilisatrice). Le
         * `pt-11` ne sert qu'à dégager la hauteur de la barre supérieure (zone de titre), pas à
         * loger un bandeau. */}
        <nav className="relative z-[51] flex h-full flex-col gap-2 px-2 pb-2 pt-11">

          <div className="no-scrollbar mask-fade-out flex grow flex-col space-y-3 overflow-x-hidden overflow-y-scroll pb-8">
            {sections.map((section, i) => (
              <div key={section.label ?? `group-${i}`} className="space-y-0.5">
                {section.label && (
                  <div className="mb-1 flex items-center px-1.5">
                    <span className="text-tiny font-semibold uppercase tracking-wide text-sidebar-ink-faint">
                      {section.label}
                    </span>
                  </div>
                )}
                {section.items.map((item) => {
                  const active = item.active ?? current === item.id;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={item.onClick ?? (() => onSelect(item.id))}
                      onContextMenu={item.onContextMenu}
                      onAuxClick={item.onAuxClick}
                      title={item.title ?? item.label}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-sm font-medium transition-colors",
                        active
                          ? "bg-accent text-white"
                          : "text-sidebar-ink-dull hover:bg-sidebar-selected/20 hover:text-sidebar-ink",
                      )}
                    >
                      <span className={cn("shrink-0", !active && item.iconClassName)}>
                        <Icon name={item.icon} size={16} />
                      </span>
                      <span className="flex-1 truncate text-left">{item.label}</span>
                    </button>
                  );
                })}
              </div>
            ))}
          </div>

          {/* Pied — même disposition que l'amont : actions à gauche, Paramètres à droite. */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <CircleButton
                icon={dark ? "light_mode" : "dark_mode"}
                size="sm"
                title={dark ? "Thème clair" : "Thème sombre"}
                aria-label={dark ? "Thème clair" : "Thème sombre"}
                onClick={() => setTheme(dark ? "light" : "dark")}
              />
              <CircleButton
                icon="search"
                size="sm"
                title="Palette de commandes (Ctrl+K)"
                aria-label="Palette de commandes"
                onClick={() =>
                  window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }))
                }
              />
              {/* Gestionnaire d'opérations — même emplacement que le `JobsButton` de la
               * `SpacesSidebar` amont (pied de barre, à côté du moniteur de sync). */}
              <JobManagerButton />
            </div>
            <div className="flex items-center gap-1">
              {onBasculerRepli && (
                <CircleButton
                  icon="chevron_left"
                  size="sm"
                  title="Replier la barre latérale (Ctrl+B)"
                  aria-label="Replier la barre latérale"
                  onClick={onBasculerRepli}
                />
              )}
              <CircleButton
                icon="settings"
                size="sm"
                title={t("tab.settings")}
                aria-label={t("tab.settings")}
                onClick={onOpenSettings}
              />
            </div>
          </div>
        </nav>
      </div>
    </div>
  );
}
