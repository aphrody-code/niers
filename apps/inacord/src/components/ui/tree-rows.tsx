// Arbre rendu en LISTE PLATE virtualisée — l'indentation est une marge, pas une imbrication DOM.
//
// `ui/collapsible.tsx` anime `height` par panneau (`transition-[height]`) : sur un T2B à 14 448
// enfants, chaque dépliage déclencherait une transition de mise en page sur des milliers de nœuds.
// Ici, déplier ne fait qu'allonger le tableau `items` que l'appelant recalcule (`flattenT2b`), et
// seule la fenêtre visible existe dans le DOM.
import { useCallback, useRef, useState, type ReactNode, type UIEvent } from "react";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export interface TreeRowItem {
  key: string;
  depth: number;
  expanded: boolean;
  hasChildren: boolean;
  label: ReactNode;
  /** Rendu à droite de la ligne (compteurs, badges…). */
  trailing?: ReactNode;
  title?: string;
}

export interface TreeRowsProps {
  items: TreeRowItem[];
  onToggle: (key: string) => void;
  onSelect?: (key: string) => void;
  selectedKey?: string | null;
  rowHeight?: number;
  height?: number;
  className?: string;
  empty?: ReactNode;
}

const OVERSCAN = 10;
const INDENT_PX = 14;

export function TreeRows({
  items,
  onToggle,
  onSelect,
  selectedKey,
  rowHeight = 24,
  height = 384,
  className,
  empty,
}: TreeRowsProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const frame = useRef<number | null>(null);

  const onScroll = useCallback((e: UIEvent<HTMLDivElement>) => {
    const next = e.currentTarget.scrollTop;
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      setScrollTop(next);
    });
  }, []);

  const visibleCount = Math.ceil(height / rowHeight) + OVERSCAN * 2;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN);
  const end = Math.min(items.length, start + visibleCount);

  const rows: ReactNode[] = [];
  for (let i = start; i < end; i++) {
    const item = items[i]!;
    const selected = selectedKey === item.key;
    rows.push(
      <div
        key={item.key}
        className={cn(
          "absolute inset-x-0 flex items-center gap-1 pr-2 text-tiny",
          selected ? "bg-app-selected text-ink" : "text-ink-dull hover:bg-app-hover",
        )}
        style={{ top: i * rowHeight, height: rowHeight, paddingLeft: item.depth * INDENT_PX }}
      >
        {item.hasChildren ? (
          <button
            type="button"
            className="flex size-4 shrink-0 items-center justify-center rounded text-ink-faint hover:text-ink"
            onClick={() => onToggle(item.key)}
            aria-label={item.expanded ? "Replier" : "Déplier"}
          >
            {/* `chevron_down` n'existe pas dans `iconMap` (Icon renvoie null en silence) — le nom
             * Material Symbols de la flèche vers le bas est `expand_more`. */}
            <Icon name={item.expanded ? "expand_more" : "chevron_right"} size={14} />
          </button>
        ) : (
          <span className="size-4 shrink-0" />
        )}
        <button
          type="button"
          className="min-w-0 flex-1 truncate text-left"
          title={item.title}
          onClick={() => (onSelect ? onSelect(item.key) : item.hasChildren && onToggle(item.key))}
        >
          {item.label}
        </button>
        {item.trailing && <span className="shrink-0 text-tiny text-ink-faint">{item.trailing}</span>}
      </div>,
    );
  }

  return (
    <div
      className={cn("overflow-auto rounded-lg border border-app-line bg-app-dark-box", className)}
      style={{ height }}
      onScroll={onScroll}
    >
      {items.length === 0 ? (
        <div className="p-3 text-xs text-ink-faint">{empty ?? "Rien à afficher."}</div>
      ) : (
        <div style={{ position: "relative", height: items.length * rowHeight }}>{rows}</div>
      )}
    </div>
  );
}
