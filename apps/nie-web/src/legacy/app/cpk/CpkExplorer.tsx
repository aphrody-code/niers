"use client";

/**
 * Explorateur /cpk — file manager **Material 3 / style Google Drive**.
 *
 *   ┌ top app bar (recherche · tri · vue) ───────────────────────────────────┐
 *   ├ onglets de dossiers ────────────────────────────────────────────────────┤
 *   │ nav rail : arbre virtualisé lazy (CpkTree) │ panneau principal adaptatif │
 *   │ surface container                          │ dossier(grille/liste) | file │
 *
 * Layout canonical M3 *list-detail* ; tient les 250 800 fichiers (virtualisation + lazy par
 * dossier). URL synchronisée (`/cpk/<path>`, deep-link). Recherche globale serveur (`/api/cpk?q`).
 * Multi-sélection (arborist) → floating toolbar M3. Viewers wasm natifs via `CpkFilePreview`.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { CpkFilePreview } from "@/app/cpk/CpkFilePreview";
import { CpkFolderView } from "@/app/cpk/CpkFolderView";
import { CpkTree } from "@/app/cpk/CpkTree";
import { cpkAssetKind, cpkAssetUrl, cpkRawUrl, cpkPreviewKind, segLabel } from "@rosegriffon/azalee/cpk/shared";
import type { CpkNode } from "@rosegriffon/azalee/cpk/tree";

interface FileSel {
	id: string;
	name: string;
	ext: string;
}

/** Fil d'Ariane M3 (text buttons) depuis un chemin d'index `data/...`. */
function Breadcrumb({ path, onCrumb }: { path: string; onCrumb: (id: string) => void }) {
	const segs = path.replace(/^data\/?/, "").split("/").filter(Boolean);
	return (
		<nav className="flex items-center gap-0.5 text-sm text-on-surface-variant overflow-x-auto min-w-0">
			<button
				type="button"
				onClick={() => onCrumb("data")}
				className="px-2 py-1 rounded-full hover:bg-on-surface/[0.08] shrink-0"
			>
				CPK
			</button>
			{segs.map((s, i) => {
				// Le dernier segment = emplacement courant (dossier ou fichier sélectionné) : NON
				// cliquable. Sinon, sur un fichier, `onCrumb(fichier)` ouvrirait un « dossier vide ».
				const isLast = i === segs.length - 1;
				return (
					<span key={i} className="flex items-center gap-0.5 shrink-0">
						<Icon name="chevron_right" size={16} className="text-on-surface-variant/50" />
						{isLast ? (
							<span className="px-2 py-1 text-on-surface font-medium">{segLabel(s)}</span>
						) : (
							<button
								type="button"
								onClick={() => onCrumb(`data/${segs.slice(0, i + 1).join("/")}`)}
								className="px-2 py-1 rounded-full hover:bg-on-surface/[0.08]"
							>
								{segLabel(s)}
							</button>
						)}
					</span>
				);
			})}
		</nav>
	);
}

export function CpkExplorer({ initialPath }: { initialPath?: string }) {
	const start = initialPath && initialPath !== "data" ? initialPath : "data";
	const startIsFile = /\.[a-z0-9]+$/i.test(start);
	const startDir = startIsFile ? start.split("/").slice(0, -1).join("/") : start;

	// Onglets de dossiers ouverts (+ dossier actif).
	const [tabs, setTabs] = useState<string[]>([startDir]);
	const [activeTab, setActiveTab] = useState(0);
	const dir = tabs[activeTab] ?? "data";

	const [selected, setSelected] = useState<FileSel | null>(
		startIsFile ? { id: start, name: start.split("/").pop() ?? start, ext: start.split(".").pop() ?? "" } : null,
	);
	const [view, setView] = useState<"grid" | "list">("grid");
	const [sort, setSort] = useState<"name" | "type">("name");
	const [multi, setMulti] = useState<CpkNode[]>([]);

	// Recherche globale (debounced + garde anti out-of-order).
	const [query, setQuery] = useState("");
	const [results, setResults] = useState<FileSel[] | null>(null);
	const searchSeq = useRef(0);
	useEffect(() => {
		const q = query.trim();
		if (q.length < 2) {
			setResults(null);
			return;
		}
		const seq = ++searchSeq.current;
		const id = setTimeout(() => {
			fetch(`/api/cpk?q=${encodeURIComponent(q)}`)
				.then((r) => (r.ok ? r.json() : null))
				.then((j) => {
					// Ignore une réponse périmée (frappe plus récente déjà partie).
					if (seq !== searchSeq.current) return;
					setResults(
						(j?.files ?? []).map((f: { name: string; ext: string; path: string }) => ({
							id: f.path,
							name: f.name,
							ext: f.ext,
						})),
					);
				})
				.catch(() => {
					if (seq === searchSeq.current) setResults([]);
				});
		}, 250);
		return () => clearTimeout(id);
	}, [query]);

	const syncUrl = useCallback((id: string) => {
		window.history.replaceState(null, "", `/cpk/${id.replace(/^data\/?/, "")}`);
	}, []);

	const openDir = useCallback(
		(id: string) => {
			setSelected(null);
			setTabs((t) => {
				const i = t.indexOf(id);
				if (i >= 0) {
					setActiveTab(i);
					return t;
				}
				setActiveTab(t.length);
				return [...t, id];
			});
			syncUrl(id);
		},
		[syncUrl],
	);

	const openFile = useCallback(
		(id: string, ext: string, name: string) => {
			// Ferme une recherche active : sinon le panneau de résultats masque la preview
			// (le rendu central teste `results !== null` AVANT `selected`).
			setQuery("");
			setResults(null);
			// Aligne l'onglet sur le dossier parent du fichier (fermer la fiche → bon dossier ;
			// vrai pour les ouvertures depuis l'arbre où le parent diffère de l'onglet actif).
			const parent = id.split("/").slice(0, -1).join("/") || "data";
			setTabs((t) => {
				const i = t.indexOf(parent);
				if (i >= 0) {
					setActiveTab(i);
					return t;
				}
				setActiveTab(t.length);
				return [...t, parent];
			});
			setSelected({ id, name, ext });
			syncUrl(id);
		},
		[syncUrl],
	);

	const onSelectFile = useCallback((node: CpkNode) => openFile(node.id, node.ext ?? "", node.name), [openFile]);
	const onSelectDir = useCallback((node: CpkNode) => openDir(node.id), [openDir]);

	const closeTab = (i: number) => {
		if (tabs.length === 1) return;
		const next = tabs.filter((_, k) => k !== i);
		const newActive =
			activeTab >= next.length ? next.length - 1 : activeTab > i ? activeTab - 1 : activeTab;
		setTabs(next);
		setActiveTab(newActive);
		// Fermer l'onglet actif → l'URL et une éventuelle fiche fichier doivent suivre le
		// nouveau dossier actif (sinon URL périmée + preview orpheline).
		if (i === activeTab) {
			setSelected(null);
			syncUrl(next[newActive] ?? "data");
		}
	};

	const selectedFiles = multi.filter((n) => n.kind === "file");

	return (
		<div className="flex flex-col h-[calc(100vh-9rem)] min-h-[34rem] rounded-3xl border border-outline-variant/20 overflow-hidden bg-surface relative">
			{/* Top app bar */}
			<div className="flex items-center gap-2 px-3 py-2 border-b border-outline-variant/20 bg-surface-container/60">
				<div className="flex items-center gap-2 flex-1 min-w-0 h-10 px-3 rounded-full bg-surface-container-highest">
					<Icon name="search" size={18} className="text-on-surface-variant shrink-0" />
					<input
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						placeholder="Rechercher un fichier dans les 250 800…"
						className="flex-1 min-w-0 bg-transparent outline-none text-sm text-on-surface placeholder:text-on-surface-variant/60"
					/>
					{query && (
						<button type="button" onClick={() => setQuery("")} className="shrink-0">
							<Icon name="close" size={18} className="text-on-surface-variant" />
						</button>
					)}
				</div>
				<select
					value={sort}
					onChange={(e) => setSort(e.target.value as "name" | "type")}
					className="h-10 px-3 rounded-full bg-surface-container-highest text-sm text-on-surface outline-none cursor-pointer"
					title="Trier"
				>
					<option value="name">Nom</option>
					<option value="type">Type</option>
				</select>
				<div className="flex h-10 rounded-full bg-surface-container-highest overflow-hidden">
					{(["grid", "list"] as const).map((v) => (
						<button
							key={v}
							type="button"
							onClick={() => setView(v)}
							className={`px-3 flex items-center transition-colors ${
								view === v ? "bg-secondary-container text-on-secondary-container" : "text-on-surface-variant hover:bg-on-surface/[0.08]"
							}`}
							title={v === "grid" ? "Grille" : "Liste"}
						>
							<Icon name={v === "grid" ? "grid_view" : "view_list"} size={18} />
						</button>
					))}
				</div>
			</div>

			{/* Onglets */}
			{tabs.length > 0 && (
				<div className="flex items-center gap-1 px-2 py-1 border-b border-outline-variant/20 bg-surface-container/30 overflow-x-auto">
					{tabs.map((t, i) => (
						<div
							key={t}
							className={`group flex items-center gap-1.5 h-8 pl-3 pr-1.5 rounded-full text-xs shrink-0 cursor-pointer transition-colors ${
								i === activeTab ? "bg-primary-container text-on-primary-container" : "text-on-surface-variant hover:bg-on-surface/[0.08]"
							}`}
							onClick={() => {
								setActiveTab(i);
								setSelected(null);
								syncUrl(tabs[i]);
							}}
						>
							<Icon name="folder" size={14} />
							<span className="max-w-[12rem] truncate">
								{t === "data" ? "CPK" : segLabel(t.split("/").pop() ?? t)}
							</span>
							{tabs.length > 1 && (
								<button
									type="button"
									onClick={(e) => {
										e.stopPropagation();
										closeTab(i);
									}}
									className="rounded-full hover:bg-on-surface/[0.12] p-0.5"
								>
									<Icon name="close" size={12} />
								</button>
							)}
						</div>
					))}
				</div>
			)}

			<div className="flex flex-1 min-h-0">
				{/* Nav rail : arbre */}
				<aside className="w-64 lg:w-72 shrink-0 bg-surface-container border-r border-outline-variant/20 flex flex-col">
					<div className="flex-1 min-h-0 p-1.5">
						<CpkTree
							initialPath={initialPath}
							onSelectFile={onSelectFile}
							onSelectDir={onSelectDir}
							onMultiSelect={setMulti}
						/>
					</div>
				</aside>

				{/* Panneau principal adaptatif */}
				<section className="flex-1 min-w-0 flex flex-col">
					<header className="px-4 py-2.5 border-b border-outline-variant/20 bg-surface">
						{results !== null ? (
							<span className="text-sm text-on-surface-variant">
								{results.length} résultat(s) pour « {query} »
							</span>
						) : (
							<Breadcrumb path={selected?.id ?? dir} onCrumb={openDir} />
						)}
					</header>
					<div className="flex-1 min-h-0 overflow-y-auto p-4 sm:p-5">
						{results !== null ? (
							<SearchResults results={results} view={view} onOpen={openFile} />
						) : selected ? (
							<CpkFilePreview
								key={selected.id}
								name={selected.name}
								ext={selected.ext}
								path={selected.id}
								kind={cpkAssetKind(selected.ext)}
								assetUrl={cpkAssetUrl(selected.id, selected.ext)}
							/>
						) : (
							<CpkFolderView dir={dir} view={view} sort={sort} onOpenDir={openDir} onOpenFile={openFile} />
						)}
					</div>
				</section>
			</div>

			{/* Floating toolbar de multi-sélection (M3) */}
			{selectedFiles.length > 1 && (
				<div className="absolute bottom-5 left-1/2 -translate-x-1/2 flex items-center gap-1 h-14 px-2 rounded-2xl bg-surface-container-high shadow-lg border border-outline-variant/20 z-20">
					<span className="px-3 text-sm font-medium text-on-surface">{selectedFiles.length} sélectionnés</span>
					<button
						type="button"
						onClick={() => {
							for (const f of selectedFiles) window.open(cpkRawUrl(f.id), "_blank");
						}}
						className="flex items-center gap-1.5 h-10 px-3 rounded-full text-sm text-on-surface hover:bg-on-surface/[0.08]"
					>
						<Icon name="download" size={18} /> Télécharger
					</button>
					<button
						type="button"
						onClick={() => navigator.clipboard?.writeText(selectedFiles.map((f) => f.id).join("\n"))}
						className="flex items-center gap-1.5 h-10 px-3 rounded-full text-sm text-on-surface hover:bg-on-surface/[0.08]"
					>
						<Icon name="content_copy" size={18} /> Copier chemins
					</button>
				</div>
			)}
		</div>
	);
}

/** Résultats de recherche (réutilise le style grille/liste, chemins complets cliquables). */
function SearchResults({
	results,
	view,
	onOpen,
}: {
	results: FileSel[];
	view: "grid" | "list";
	onOpen: (id: string, ext: string, name: string) => void;
}) {
	if (results.length === 0) {
		return <p className="p-8 text-center text-sm text-on-surface-variant">Aucun fichier.</p>;
	}
	const icon = (ext: string) => {
		switch (cpkPreviewKind(ext)) {
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
	};
	return (
		<ul className={view === "grid" ? "grid grid-cols-1 md:grid-cols-2 gap-1" : "divide-y divide-outline-variant/10"}>
			{results.map((f) => (
				<li key={f.id}>
					<button
						type="button"
						onClick={() => onOpen(f.id, f.ext, f.name)}
						className="w-full flex items-center gap-3 h-12 px-2 rounded-lg text-left hover:bg-on-surface/[0.08] transition-colors"
					>
						<Icon name={icon(f.ext)} size={20} className="shrink-0 text-on-surface-variant" />
						<span className="flex-1 min-w-0">
							<span className="block truncate text-sm text-on-surface">{f.name}</span>
							<span className="block truncate text-[11px] font-mono text-on-surface-variant/60">
								{f.id.replace(/^data\//, "")}
							</span>
						</span>
					</button>
				</li>
			))}
		</ul>
	);
}
