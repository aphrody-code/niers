// Visionneuse structurée des `.cfg.bin` décodés — table et arbre, en bascule avec Monaco.
//
// Le JSON « inagle » produit par `vfs_decode_cfgbin` est lisible mais pas exploitable : un T2B de
// personnages fait 1,38 Mo pour 14 448 entrées de 34 variables, et un RDBN est une table dont
// Monaco n'affiche qu'une ligne de clé/valeur par écran. Les deux vues ajoutées ici lisent et
// reproduisent EXACTEMENT la même chaîne JSON que l'éditeur texte : c'est la même donnée, le même
// chemin d'enregistrement (`encode_cfgbin_config`), aucune commande Tauri nouvelle.
//
// Garde-fou d'intégrité : la bascule n'est éditable que si `stringifyJson(parseJson(texte))`
// reproduit le texte au caractère près (`CfgDoc.faithful`). Sinon les vues restent en LECTURE
// SEULE — passer par la table ne doit jamais réécrire un octet que personne n'a touché.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import Editor from "@monaco-editor/react";
import { useTheme } from "next-themes";
import { toast } from "sonner";

import { DataGrid, type DataGridColumn } from "@/components/ui/data-grid";
import { TreeRows, type TreeRowItem } from "@/components/ui/tree-rows";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { installMonacoOffline } from "@/lib/monacoSetup";
import { cn } from "@/lib/utils";
import {
  cellEditable,
  cellTypeHint,
  detectT2bTable,
  findT2bTables,
  flattenT2b,
  formatCell,
  getAt,
  parseConfig,
  rdbnCell,
  rdbnLists,
  serializeConfig,
  setRdbnCell,
  setT2bVar,
  sortRdbnRows,
  t2bChildVar,
  t2bEntryAt,
  t2bPath,
  type CfgFormat,
  type EditResult,
} from "@/lib/cfgbinModel";

installMonacoOffline();

type Mode = "json" | "table" | "tree";

const GRID_HEIGHT = 384;

/** Cellule éditable : bouton tant qu'elle est au repos, `<input>` pendant la saisie. */
function GridCell({
  text,
  title,
  editable,
  active,
  draft,
  onDraft,
  onStart,
  onCommit,
  onCancel,
}: {
  text: string;
  title?: string;
  editable: boolean;
  active: boolean;
  draft: string;
  onDraft: (v: string) => void;
  onStart: () => void;
  onCommit: () => void;
  onCancel: () => void;
}) {
  if (active) {
    return (
      <input
        ref={(el) => {
          el?.focus();
        }}
        value={draft}
        spellCheck={false}
        onChange={(e) => onDraft(e.target.value)}
        onBlur={onCommit}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onCommit();
          } else if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
        className="h-5 w-full rounded border border-accent bg-app-input px-1 font-mono text-tiny text-ink outline-none"
      />
    );
  }
  return (
    <button
      type="button"
      disabled={!editable}
      onClick={onStart}
      title={title}
      className={cn(
        "block w-full truncate px-1 text-left font-mono text-tiny",
        editable ? "text-ink-dull hover:bg-app-box hover:text-ink" : "cursor-default text-ink-faint",
      )}
    >
      {text === "" ? "—" : text}
    </button>
  );
}

export interface CfgbinViewerProps {
  /** JSON décodé, tel que `JSON.stringify(value, null, 2)` — la chaîne que consomme Monaco. */
  value: string;
  /** Reçoit la MÊME forme de chaîne après édition. */
  onChange: (next: string) => void;
  format: CfgFormat;
  readOnly?: boolean;
  className?: string;
}

export function CfgbinViewer({ value, onChange, format, readOnly = false, className }: CfgbinViewerProps) {
  const { resolvedTheme } = useTheme();
  const [mode, setMode] = useState<Mode>("json");
  const [listIndex, setListIndex] = useState(0);
  const [tableIndex, setTableIndex] = useState(0);
  const [sort, setSort] = useState<{ key: string; dir: "asc" | "desc" } | null>(null);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ row: number; col: string } | null>(null);
  const [draft, setDraft] = useState("");

  const parsed = useMemo(() => parseConfig(value, format), [value, format]);
  const doc = parsed.ok ? parsed.doc : null;
  const root = doc?.root ?? null;

  // Un JSON laissé invalide dans Monaco ne doit pas piéger l'utilisatrice dans un onglet vide.
  const effectiveMode: Mode = doc ? mode : "json";

  // Changer de fichier remet la sélection à zéro : les index de liste/table ne veulent rien dire
  // d'un document à l'autre.
  useEffect(() => {
    setListIndex(0);
    setTableIndex(0);
    setSort(null);
    setExpanded(new Set<string>());
    setSelectedKey(null);
    setEditing(null);
  }, [format]);

  const editable = !readOnly && doc !== null && doc.faithful;

  const lists = useMemo(() => (root && format === "rdbn" ? rdbnLists(root) : []), [root, format]);
  const tables = useMemo(() => (root && format === "t2b" ? findT2bTables(root) : []), [root, format]);

  const list = lists[Math.min(listIndex, Math.max(0, lists.length - 1))] ?? null;
  const table = tables[Math.min(tableIndex, Math.max(0, tables.length - 1))] ?? null;

  const order = useMemo(() => {
    if (!root || format !== "rdbn" || !list) return undefined;
    if (!sort) return undefined;
    return sortRdbnRows(root, listIndex, list.rowCount, sort.key, sort.dir);
  }, [root, format, list, listIndex, sort]);

  const treeItems = useMemo<TreeRowItem[]>(() => {
    if (!root || format !== "t2b") return [];
    return flattenT2b(root, expanded).map((r) => ({
      key: r.key,
      depth: r.depth,
      expanded: r.expanded,
      hasChildren: r.childCount > 0,
      title: r.name,
      label: <span className="font-mono">{r.name}</span>,
      trailing:
        r.childCount > 0
          ? `${r.childCount} enfant(s)`
          : r.varCount > 0
            ? `${r.varCount} var.`
            : undefined,
    }));
  }, [root, format, expanded]);

  const selectedIndices = useMemo(
    () => (selectedKey ? selectedKey.split("/").map(Number) : null),
    [selectedKey],
  );

  /** Applique une écriture du modèle et remonte la nouvelle chaîne, ou explique le refus. */
  function apply(result: EditResult) {
    setEditing(null);
    if (!result.ok) {
      toast.error(result.error);
      return;
    }
    if (!doc) return;
    onChange(serializeConfig({ ...doc, root: result.root }));
  }

  function toggle(key: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  // --- RDBN : une liste = une table -----------------------------------------------------------
  const rdbnColumns = useMemo<DataGridColumn[]>(
    () =>
      (list?.columns ?? []).map((c) => ({
        key: c,
        label: c,
        width: "9rem",
        title: root ? `${c} · ${cellTypeHint(rdbnCell(root, listIndex, 0, c))}` : c,
      })),
    [list, root, listIndex],
  );

  function renderRdbnCell(row: number, column: DataGridColumn): ReactNode {
    if (!root) return null;
    const node = rdbnCell(root, listIndex, row, column.key);
    const active = editing?.row === row && editing.col === column.key;
    const cellIsEditable = editable && cellEditable(node);
    return (
      <GridCell
        text={formatCell(node)}
        title={cellTypeHint(node)}
        editable={cellIsEditable}
        active={active}
        draft={draft}
        onDraft={setDraft}
        onStart={() => {
          setEditing({ row, col: column.key });
          setDraft(formatCell(node));
        }}
        onCommit={() => apply(setRdbnCell(root, listIndex, row, column.key, draft))}
        onCancel={() => setEditing(null)}
      />
    );
  }

  // --- T2B : enfants homogènes = une table ----------------------------------------------------
  const t2bColumns = useMemo<DataGridColumn[]>(
    () =>
      (table?.table.columns ?? []).map((c) => ({
        // Le format T2B ne porte AUCUN nom de colonne : les variables sont positionnelles, seule
        // leur position et leur type sont connus.
        key: `v${c.index}`,
        label: `#${c.index} · ${c.type}`,
        width: c.type === "String" ? "12rem" : "7rem",
      })),
    [table],
  );

  function renderT2bTableCell(row: number, column: DataGridColumn): ReactNode {
    if (!root || !table) return null;
    const varIndex = Number(column.key.slice(1));
    const view = t2bChildVar(root, table.indices, row, varIndex);
    const active = editing?.row === row && editing.col === column.key;
    return (
      <GridCell
        text={view?.value ?? ""}
        title={view ? `${view.type}${view.malformed ? " · valeur non textuelle, non éditable" : ""}` : undefined}
        editable={editable && view !== null && !view.malformed}
        active={active}
        draft={draft}
        onDraft={setDraft}
        onStart={() => {
          setEditing({ row, col: column.key });
          setDraft(view?.value ?? "");
        }}
        onCommit={() => apply(setT2bVar(root, [...table.indices, row], varIndex, draft))}
        onCancel={() => setEditing(null)}
      />
    );
  }

  // --- T2B : détail du nœud sélectionné dans l'arbre -------------------------------------------
  const selectedNode = useMemo(
    () => (root && selectedIndices ? getAt(root, t2bPath(selectedIndices)) : null),
    [root, selectedIndices],
  );
  const selectedTable = useMemo(() => (selectedNode ? detectT2bTable(selectedNode) : null), [selectedNode]);
  const selectedEntry = useMemo(
    () => (root && selectedIndices ? t2bEntryAt(root, selectedIndices) : null),
    [root, selectedIndices],
  );

  const detailColumns = useMemo<DataGridColumn[]>(() => {
    if (selectedTable) {
      return selectedTable.columns.map((c) => ({
        key: `v${c.index}`,
        label: `#${c.index} · ${c.type}`,
        width: c.type === "String" ? "12rem" : "7rem",
      }));
    }
    return [
      { key: "type", label: "Type", width: "6rem" },
      { key: "value", label: "Valeur", width: "minmax(14rem, 1fr)" },
    ];
  }, [selectedTable]);

  function renderDetailCell(row: number, column: DataGridColumn): ReactNode {
    if (!root || !selectedIndices) return null;
    if (selectedTable) {
      const varIndex = Number(column.key.slice(1));
      const view = t2bChildVar(root, selectedIndices, row, varIndex);
      const active = editing?.row === row && editing.col === column.key;
      return (
        <GridCell
          text={view?.value ?? ""}
          title={view?.type}
          editable={editable && view !== null && !view.malformed}
          active={active}
          draft={draft}
          onDraft={setDraft}
          onStart={() => {
            setEditing({ row, col: column.key });
            setDraft(view?.value ?? "");
          }}
          onCommit={() => apply(setT2bVar(root, [...selectedIndices, row], varIndex, draft))}
          onCancel={() => setEditing(null)}
        />
      );
    }
    const v = selectedEntry?.vars[row];
    if (!v) return null;
    if (column.key === "type") return <span className="px-1 font-mono text-tiny text-ink-faint">{v.type}</span>;
    const active = editing?.row === row && editing.col === column.key;
    return (
      <GridCell
        text={v.value}
        title={v.malformed ? "valeur non textuelle dans le JSON — non éditable" : v.type}
        editable={editable && !v.malformed}
        active={active}
        draft={draft}
        onDraft={setDraft}
        onStart={() => {
          setEditing({ row, col: column.key });
          setDraft(v.value);
        }}
        onCommit={() => apply(setT2bVar(root, selectedIndices, row, draft))}
        onCancel={() => setEditing(null)}
      />
    );
  }

  return (
    <div className={cn("flex min-h-0 flex-col gap-1", className)}>
      <div className="flex flex-wrap items-center gap-2">
        <ToggleGroup
          value={[effectiveMode]}
          onValueChange={(v: string[]) => v[0] && setMode(v[0] as Mode)}
          className="shrink-0"
        >
          <ToggleGroupItem value="json">JSON</ToggleGroupItem>
          <ToggleGroupItem value="table" disabled={!doc}>
            Table
          </ToggleGroupItem>
          {format === "t2b" && (
            <ToggleGroupItem value="tree" disabled={!doc}>
              Arbre
            </ToggleGroupItem>
          )}
        </ToggleGroup>
        {!parsed.ok && (
          <span className="min-w-0 flex-1 truncate text-tiny text-status-error" title={parsed.error}>
            JSON invalide — vues structurées désactivées : {parsed.error}
          </span>
        )}
        {doc && !doc.faithful && (
          <span className="min-w-0 flex-1 text-tiny text-status-warning">
            Vues structurées en lecture seule : la ré-écriture de ce JSON ne le reproduit pas à
            l'identique, l'éditer par la table modifierait des valeurs non touchées. Passez par
            l'onglet JSON.
          </span>
        )}
        {doc && doc.faithful && readOnly && (
          <span className="text-tiny text-ink-faint">Lecture seule.</span>
        )}
      </div>

      {effectiveMode === "json" && (
        <div className="h-96 overflow-hidden rounded-lg border border-app-line bg-app-dark-box">
          <Editor
            height="100%"
            language="json"
            theme={resolvedTheme === "light" ? "light" : "vs-dark"}
            value={value}
            onChange={(v) => onChange(v ?? "")}
            options={{ minimap: { enabled: false }, fontSize: 12, automaticLayout: true, scrollBeyondLastLine: false }}
          />
        </div>
      )}

      {effectiveMode === "table" && format === "rdbn" && (
        <div className="flex min-h-0 flex-col gap-1">
          <div className="flex flex-wrap gap-1">
            {lists.map((l, i) => (
              <button
                key={`${l.name}-${i}`}
                type="button"
                className={cn(
                  "rounded-md px-2 py-0.5 text-tiny transition-colors",
                  i === listIndex ? "bg-accent text-white" : "bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink",
                )}
                title={`${l.typeName} · ${l.rowCount} ligne(s) · ${l.columns.length} champ(s)`}
                onClick={() => {
                  setListIndex(i);
                  setSort(null);
                  setEditing(null);
                }}
              >
                {l.name} · {l.rowCount}
              </button>
            ))}
          </div>
          <DataGrid
            columns={rdbnColumns}
            rowCount={list?.rowCount ?? 0}
            cell={renderRdbnCell}
            order={order}
            rowHeader={(row) => row}
            rowHeaderWidth="3.5rem"
            sort={sort}
            onSortChange={(key) =>
              // Tri d'AFFICHAGE seulement : `json_to_rdbn_lists` patche les lignes dans l'ordre du
              // document, une permutation réelle réécrirait chaque ligne.
              setSort((prev) =>
                prev?.key === key ? (prev.dir === "asc" ? { key, dir: "desc" } : null) : { key, dir: "asc" },
              )
            }
            height={GRID_HEIGHT}
            empty="Cette liste ne contient aucune ligne."
          />
          <p className="text-tiny text-ink-faint">
            Le tri est un ordre d'affichage : le document conserve l'ordre d'origine, seul éditable
            (le ré-encodage RDBN est un patch de valeurs, sans ajout ni suppression de ligne).
          </p>
        </div>
      )}

      {effectiveMode === "table" && format === "t2b" && (
        <div className="flex min-h-0 flex-col gap-1">
          {tables.length === 0 ? (
            <div className="rounded-lg border border-app-line bg-app-dark-box p-3 text-xs text-ink-faint">
              Aucun groupe d'enfants homogène dans ce fichier : sans séquence de types identique
              d'un enfant à l'autre, une grille alignerait des colonnes qui n'ont rien en commun.
              Utilisez l'onglet Arbre.
            </div>
          ) : (
            <>
              <div className="flex flex-wrap gap-1">
                {tables.map((c, i) => (
                  <button
                    key={c.key}
                    type="button"
                    className={cn(
                      "rounded-md px-2 py-0.5 text-tiny transition-colors",
                      i === tableIndex
                        ? "bg-accent text-white"
                        : "bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink",
                    )}
                    title={`${c.label} · ${c.table.rowCount} ligne(s) · ${c.table.columns.length} variable(s)`}
                    onClick={() => {
                      setTableIndex(i);
                      setEditing(null);
                    }}
                  >
                    {c.label} · {c.table.rowCount}
                  </button>
                ))}
              </div>
              <DataGrid
                columns={t2bColumns}
                rowCount={table?.table.rowCount ?? 0}
                cell={renderT2bTableCell}
                rowHeader={(row) => row}
                rowHeaderWidth="3.5rem"
                height={GRID_HEIGHT}
              />
              <p className="text-tiny text-ink-faint">
                Colonnes positionnelles : le format T2B ne transporte aucun nom de champ, seuls
                l'index et le type de chaque variable sont connus.
              </p>
            </>
          )}
        </div>
      )}

      {effectiveMode === "tree" && format === "t2b" && (
        <div className="flex min-h-0 flex-col gap-1">
          <TreeRows
            items={treeItems}
            onToggle={toggle}
            onSelect={(key) => {
              setSelectedKey(key);
              setEditing(null);
            }}
            selectedKey={selectedKey}
            height={240}
            empty="Ce fichier ne contient aucune entrée."
          />
          {selectedEntry ? (
            <>
              <p className="truncate text-tiny text-ink-faint">
                {selectedEntry.name} ·{" "}
                {selectedTable
                  ? `${selectedTable.rowCount} enfant(s) homogène(s), ${selectedTable.columns.length} variable(s)`
                  : `${selectedEntry.vars.length} variable(s), ${selectedEntry.childCount} enfant(s)`}
              </p>
              <DataGrid
                columns={detailColumns}
                rowCount={selectedTable ? selectedTable.rowCount : selectedEntry.vars.length}
                cell={renderDetailCell}
                rowHeader={(row) => row}
                rowHeaderWidth="3.5rem"
                height={220}
                empty="Cette entrée ne porte aucune variable."
              />
            </>
          ) : (
            <p className="text-tiny text-ink-faint">Sélectionnez une entrée pour voir ses variables.</p>
          )}
        </div>
      )}
    </div>
  );
}
