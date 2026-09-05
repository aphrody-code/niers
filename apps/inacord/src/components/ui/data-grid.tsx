// Grille de données virtualisée — briques manquantes du design system (aucun `table.tsx` n'existe).
//
// Volumétrie réelle mesurée : `common/gamedata/character/chara_base_1.03.98.00.cfg.bin` est un T2B
// de 1,38 Mo dont une entrée porte 14 448 enfants de 34 variables. Un `<table>` HTML complet ferait
// ~490 000 cellules DOM. D'où : une grille CSS, une fenêtre glissante calculée sur `scrollTop`, une
// hauteur de ligne FIXE (seule façon de connaître la position d'une ligne sans la mesurer) et aucune
// dépendance ajoutée.
import { useCallback, useMemo, useRef, useState, type ReactNode, type UIEvent } from "react";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export interface DataGridColumn {
  key: string;
  label: string;
  /** Largeur CSS de la colonne dans `grid-template-columns`. */
  width?: string;
  title?: string;
}

export interface DataGridProps {
  columns: DataGridColumn[];
  rowCount: number;
  /** Rendu d'une cellule. `row` est l'index DOCUMENT, pas l'index d'affichage. */
  cell: (row: number, column: DataGridColumn) => ReactNode;
  /** Ordre d'AFFICHAGE : `order[i]` est l'index document de la i-ème ligne visible. */
  order?: number[];
  /** Colonne figée à gauche (numéro de ligne, nom…). */
  rowHeader?: (row: number, displayIndex: number) => ReactNode;
  rowHeaderLabel?: string;
  rowHeaderWidth?: string;
  sort?: { key: string; dir: "asc" | "desc" } | null;
  onSortChange?: (key: string) => void;
  rowHeight?: number;
  height?: number;
  className?: string;
  /** Affiché à la place des lignes quand `rowCount === 0`. */
  empty?: ReactNode;
}

const OVERSCAN = 8;

export function DataGrid({
  columns,
  rowCount,
  cell,
  order,
  rowHeader,
  rowHeaderLabel = "#",
  rowHeaderWidth = "5rem",
  sort,
  onSortChange,
  rowHeight = 26,
  height = 384,
  className,
  empty,
}: DataGridProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const frame = useRef<number | null>(null);

  // Le `scrollTop` brut redéclenche un rendu à chaque pixel : on le coalesce sur la frame.
  const onScroll = useCallback((e: UIEvent<HTMLDivElement>) => {
    const next = e.currentTarget.scrollTop;
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      setScrollTop(next);
    });
  }, []);

  const template = useMemo(
    () => [rowHeader ? rowHeaderWidth : null, ...columns.map((c) => c.width ?? "10rem")].filter(Boolean).join(" "),
    [columns, rowHeader, rowHeaderWidth],
  );

  const visibleCount = Math.ceil(height / rowHeight) + OVERSCAN * 2;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN);
  const end = Math.min(rowCount, start + visibleCount);

  const rows: ReactNode[] = [];
  for (let display = start; display < end; display++) {
    const docRow = order ? (order[display] ?? display) : display;
    rows.push(
      <div
        key={docRow}
        className="absolute inset-x-0 grid items-center border-b border-app-line/50 text-tiny text-ink-dull hover:bg-app-hover"
        style={{ top: display * rowHeight, height: rowHeight, gridTemplateColumns: template }}
      >
        {rowHeader && (
          <div className="truncate px-2 font-mono text-ink-faint" title={String(docRow)}>
            {rowHeader(docRow, display)}
          </div>
        )}
        {columns.map((c) => (
          <div key={c.key} className="min-w-0 truncate px-1">
            {cell(docRow, c)}
          </div>
        ))}
      </div>,
    );
  }

  return (
    <div
      className={cn("overflow-auto rounded-lg border border-app-line bg-app-dark-box", className)}
      style={{ height }}
      onScroll={onScroll}
    >
      <div style={{ minWidth: "max-content" }}>
        <div
          className="sticky top-0 z-10 grid items-center border-b border-app-line bg-app-box text-tiny font-semibold uppercase tracking-wide text-ink-faint"
          style={{ height: rowHeight, gridTemplateColumns: template }}
        >
          {rowHeader && <div className="truncate px-2">{rowHeaderLabel}</div>}
          {columns.map((c) => (
            <button
              key={c.key}
              type="button"
              disabled={!onSortChange}
              onClick={() => onSortChange?.(c.key)}
              title={c.title ?? c.label}
              className={cn(
                "flex min-w-0 items-center gap-1 px-1 text-left uppercase",
                onSortChange ? "hover:text-ink" : "cursor-default",
              )}
            >
              <span className="min-w-0 truncate">{c.label}</span>
              {sort?.key === c.key && (
                <Icon name={sort.dir === "asc" ? "expand_less" : "expand_more"} size={12} className="shrink-0" />
              )}
            </button>
          ))}
        </div>
        {rowCount === 0 ? (
          <div className="p-3 text-xs text-ink-faint">{empty ?? "Aucune ligne."}</div>
        ) : (
          <div style={{ position: "relative", height: rowCount * rowHeight }}>{rows}</div>
        )}
      </div>
    </div>
  );
}
