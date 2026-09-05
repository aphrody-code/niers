// Rangée d'onglets de l'Explorateur — un onglet = un contexte de navigation complet
// (`lib/explorerTabs.ts`). Purement présentationnelle : elle ne connaît que le tableau d'onglets
// et remonte les gestes.
import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";
import type { ExplorerTab } from "@/lib/explorerTabs";

/** Libellé d'un onglet — dernier segment du préfixe, même règle que la barre latérale. La racine
 * VFS n'a pas de segment : elle se nomme. */
export function tabLabel(prefix: string): string {
  return prefix.split("/").pop() || prefix || "Racine";
}

export function ExplorerTabsBar({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNew,
}: {
  tabs: ExplorerTab[];
  activeId: string;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
}) {
  const closable = tabs.length > 1;

  return (
    <div className="flex shrink-0 items-center gap-1 border-b border-app-line px-2 pb-1 pt-0.5">
      <div className="no-scrollbar flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {tabs.map((tab) => {
          const active = tab.id === activeId;
          return (
            // `div role="button"` et non `<button>` : un bouton imbriqué dans un bouton est du
            // HTML invalide, et le navigateur remonte alors la croix HORS de l'onglet (bug déjà
            // vécu sur ce dépôt).
            <div
              key={tab.id}
              role="button"
              tabIndex={0}
              title={tab.prefix || "/"}
              onClick={() => onActivate(tab.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onActivate(tab.id);
                }
              }}
              // Clic milieu = fermer, comme un navigateur : React ne le voit PAS dans `onClick`.
              onAuxClick={(e) => {
                if (e.button === 1 && closable) {
                  e.preventDefault();
                  onClose(tab.id);
                }
              }}
              className={cn(
                "group flex max-w-[180px] shrink-0 cursor-default items-center gap-1.5 rounded-t-lg px-2.5 py-1 text-xs transition-colors",
                active
                  ? "bg-app-box text-ink"
                  : "text-ink-dull hover:bg-app-hover hover:text-ink",
              )}
            >
              <Icon name="folder" size={13} className={cn("shrink-0", active ? "text-accent" : "")} />
              <span className="min-w-0 truncate">{tabLabel(tab.prefix)}</span>
              {closable && (
                <button
                  type="button"
                  aria-label={`Fermer l'onglet ${tabLabel(tab.prefix)}`}
                  title="Fermer l'onglet (Ctrl+W)"
                  className="ml-0.5 flex size-4 shrink-0 items-center justify-center rounded opacity-0 transition-opacity hover:bg-app-selected group-hover:opacity-100"
                  onClick={(e) => {
                    e.stopPropagation();
                    onClose(tab.id);
                  }}
                >
                  <Icon name="close" size={12} />
                </button>
              )}
            </div>
          );
        })}
      </div>
      <button
        type="button"
        aria-label="Nouvel onglet"
        title="Nouvel onglet (Ctrl+T)"
        className="flex size-6 shrink-0 items-center justify-center rounded-md text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
        onClick={onNew}
      >
        <Icon name="add" size={14} />
      </button>
    </div>
  );
}
