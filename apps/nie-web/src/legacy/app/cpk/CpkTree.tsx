"use client";

/**
 * Arbre de navigation **virtualisé + lazy** de l'index CPK (react-arborist).
 *
 * - Virtualisation react-window : tient les 250 800 fichiers (seuls ~30 nœuds rendus).
 * - Lazy par dossier : `onToggle` fetch `/api/cpk?path=` à la 1re ouverture (cf. lib/cpk/tree).
 * - Style Material 3 : rangées 36px, state-layer hover/focus, sélection `primary container`,
 *   icônes typées par extension. Fond `surface container` (nav rail M3).
 */
import { Tree, type NodeRendererProps, type TreeApi } from "react-arborist";
import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkPreviewKind } from "@rosegriffon/azalee/cpk/shared";
import {
	type CpkNode,
	ancestorPaths,
	fetchChildren,
	isUnloaded,
	setChildren,
} from "@rosegriffon/azalee/cpk/tree";

/** Icône Material Symbols selon le type de nœud / l'extension. */
function nodeIcon(node: CpkNode, open: boolean): string {
	if (node.kind === "dir") return open ? "folder_open" : "folder";
	if (node.kind === "more") return "more_horiz";
	switch (cpkPreviewKind(node.ext ?? "")) {
		case "image":
			return "image";
		case "model":
			return "deployed_code";
		case "sound":
			return "music_note";
		case "movie":
			return "movie";
		case "config":
		case "package":
			return "data_object";
		case "text":
			return "description";
		default:
			return "draft";
	}
}

/** Rangée M3 d'un nœud (indentation via `style` arborist). */
function Row({ node, style, dragHandle }: NodeRendererProps<CpkNode>) {
	const data = node.data;
	const selected = node.isSelected;
	return (
		<div
			ref={dragHandle}
			style={style}
			// Dossier : déplie ET ouvre dans le panneau central (sinon le clic ne faisait que
			// déplier l'arbre, sans jamais montrer le contenu). Fichier/sentinelle : active.
			onClick={() => {
				if (node.isInternal) node.toggle();
				node.activate();
			}}
			className={`group flex items-center gap-1.5 h-9 pr-2 rounded-full cursor-pointer select-none text-sm transition-colors ${
				selected
					? "bg-primary-container text-on-primary-container"
					: "text-on-surface hover:bg-on-surface/[0.08]"
			}`}
		>
			{node.isInternal ? (
				<Icon
					name={node.isOpen ? "expand_more" : "chevron_right"}
					size={18}
					className="shrink-0 text-on-surface-variant"
				/>
			) : (
				<span className="w-[18px] shrink-0" />
			)}
			<Icon
				name={nodeIcon(data, node.isOpen)}
				size={18}
				className={`shrink-0 ${selected ? "text-on-primary-container" : "text-on-surface-variant"}`}
			/>
			<span className={`truncate flex-1 ${data.kind === "more" ? "italic text-on-surface-variant/80" : ""}`}>
				{data.name}
			</span>
			{data.kind === "dir" && typeof data.count === "number" && (
				<span className="shrink-0 text-[11px] tabular-nums text-on-surface-variant/70">
					{data.count.toLocaleString("fr")}
				</span>
			)}
		</div>
	);
}

export function CpkTree({
	initialPath,
	onSelectFile,
	onSelectDir,
	onMultiSelect,
}: {
	initialPath?: string;
	onSelectFile?: (node: CpkNode) => void;
	onSelectDir?: (node: CpkNode) => void;
	onMultiSelect?: (nodes: CpkNode[]) => void;
}) {
	const [data, setData] = useState<CpkNode[]>([]);
	const [size, setSize] = useState({ w: 320, h: 600 });
	const boxRef = useRef<HTMLDivElement>(null);
	const treeRef = useRef<TreeApi<CpkNode> | null>(null);
	const revealedRef = useRef(false);
	// Évite les états périmés dans les callbacks async.
	const dataRef = useRef<CpkNode[]>([]);
	dataRef.current = data;

	// Une fois les ancêtres chargés, révèle + scrolle vers le fichier deep-linké (une seule fois).
	useEffect(() => {
		if (revealedRef.current || !initialPath || initialPath === "data" || data.length === 0) return;
		const t = treeRef.current;
		if (!t) return;
		for (const anc of ancestorPaths(initialPath).slice(0, -1)) t.open(anc);
		if (/\.[a-z0-9]+$/i.test(initialPath)) {
			t.select(initialPath);
			t.scrollTo(initialPath, "center");
		}
		revealedRef.current = true;
	}, [data, initialPath]);

	// Dimensionne l'arbre à son conteneur (react-arborist exige width/height).
	useEffect(() => {
		const el = boxRef.current;
		if (!el) return;
		const ro = new ResizeObserver(() => {
			setSize({ w: el.clientWidth, h: el.clientHeight });
		});
		ro.observe(el);
		return () => ro.disconnect();
	}, []);

	// Charge la racine, puis auto-déplie les ancêtres d'un deep-link.
	useEffect(() => {
		let cancelled = false;
		(async () => {
			const roots = await fetchChildren("data");
			if (cancelled) return;
			let tree = roots;
			// Auto-expansion progressive vers initialPath.
			if (initialPath && initialPath !== "data") {
				for (const anc of ancestorPaths(initialPath)) {
					const kids = await fetchChildren(anc);
					if (cancelled) return;
					tree = setChildren(tree, anc, kids);
				}
			}
			setData(tree);
		})();
		return () => {
			cancelled = true;
		};
	}, [initialPath]);

	const onToggle = useCallback(async (id: string) => {
		// react-arborist appelle onToggle APRÈS le changement d'état d'ouverture.
		const { findNode } = await import("@rosegriffon/azalee/cpk/tree");
		const node = findNode(dataRef.current, id);
		if (node && isUnloaded(node)) {
			const kids = await fetchChildren(id);
			setData((d) => setChildren(d, id, kids));
		}
	}, []);

	const onActivate = useCallback(
		(node: { data: CpkNode }) => {
			const d = node.data;
			if (d.kind === "file") onSelectFile?.(d);
			// Sentinelle « … N de plus » → ouvre le dossier parent dans la grille paginée.
			else if (d.kind === "more") onSelectDir?.({ id: d.parent ?? "data", name: "", kind: "dir" });
			else onSelectDir?.(d);
		},
		[onSelectFile, onSelectDir],
	);

	return (
		<div ref={boxRef} className="h-full w-full">
			<Tree<CpkNode>
				ref={treeRef}
				data={data}
				idAccessor="id"
				childrenAccessor="children"
				openByDefault={false}
				rowHeight={36}
				indent={14}
				width={size.w}
				height={size.h}
				disableDrag
				disableEdit
				onToggle={onToggle}
				onActivate={onActivate}
				onSelect={(nodes) => onMultiSelect?.(nodes.map((n) => n.data))}
			>
				{Row}
			</Tree>
		</div>
	);
}
