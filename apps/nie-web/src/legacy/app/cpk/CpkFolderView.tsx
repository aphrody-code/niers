"use client";

/**
 * Vue du contenu d'un dossier CPK — **grille** (elevated cards M3 12dp, thumbnails) ou **liste**
 * (M3 list items), avec **tri** (nom / type). Sous-dossiers (toujours complets) + fichiers du
 * dossier courant **paginés côté serveur** (`/api/cpk?path=&limit=&offset=`). Clic dossier →
 * navigation ; clic fichier → preview.
 *
 * Pagination serveur réelle (et non un simple `slice` client) : un répertoire peut compter des
 * dizaines de milliers de fichiers directs (`common/sound/ja` ≈ 15 443) — « Afficher plus » fetch
 * la **page suivante** depuis le serveur au lieu de tout télécharger d'un coup.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { CpkModelThumb } from "@/app/cpk/CpkModelThumb";
import { Icon } from "@/components/ui/Icon";
import { cpkPreviewKind, cpkThumbUrl } from "@rosegriffon/azalee/cpk/shared";

/** Taille de page de fichiers (doit rester ≤ MAX_LIMIT de l'API). */
const PAGE = 500;
const CDN = "https://cdn.rosegriffon.fr";
// Sous-domaines `common/chr/_<sub>/` dont chaque sous-dossier est un modèle servable en GLB
// (route `/model-chr/<sub>/<code>`). Quand on liste un tel dossier, on rend une galerie 3D.
const MODEL_PARENT_SUBS = new Set(["waza", "item", "animal", "keshin", "armd"]);

interface Item {
	id: string;
	name: string;
	kind: "dir" | "file";
	ext?: string;
	count?: number;
}

interface ApiListing {
	dir: string;
	dirs: { name: string; count: number }[];
	files: { name: string; ext: string; path: string }[];
	fileTotal?: number;
}

function itemIcon(it: Item): string {
	if (it.kind === "dir") return "folder";
	switch (cpkPreviewKind(it.ext ?? "")) {
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

export function CpkFolderView({
	dir,
	view,
	sort,
	onOpenDir,
	onOpenFile,
}: {
	dir: string;
	view: "grid" | "list";
	sort: "name" | "type";
	onOpenDir: (id: string) => void;
	onOpenFile: (id: string, ext: string, name: string) => void;
}) {
	const [dirs, setDirs] = useState<Item[]>([]);
	const [files, setFiles] = useState<Item[]>([]);
	const [fileTotal, setFileTotal] = useState(0);
	const [loading, setLoading] = useState(true);
	const [loadingMore, setLoadingMore] = useState(false);
	// Identité du dossier courant : invalide les réponses de fetch périmées (navigation rapide).
	const dirRef = useRef(dir);
	dirRef.current = dir;

	useEffect(() => {
		let on = true;
		setLoading(true);
		setDirs([]);
		setFiles([]);
		setFileTotal(0);
		fetch(`/api/cpk?path=${encodeURIComponent(dir)}&limit=${PAGE}`)
			.then((r) => (r.ok ? (r.json() as Promise<ApiListing>) : null))
			.then((j) => {
				if (!on) return;
				if (j) {
					setDirs((j.dirs ?? []).map((d) => ({ id: `${dir}/${d.name}`, name: d.name, kind: "dir", count: d.count })));
					setFiles((j.files ?? []).map((f) => ({ id: f.path, name: f.name, kind: "file", ext: f.ext })));
					setFileTotal(j.fileTotal ?? (j.files?.length ?? 0));
				}
				setLoading(false);
			})
			.catch(() => on && setLoading(false));
		return () => {
			on = false;
		};
	}, [dir]);

	const loadMore = () => {
		if (loadingMore || files.length >= fileTotal) return;
		const offset = files.length;
		const base = dir;
		setLoadingMore(true);
		fetch(`/api/cpk?path=${encodeURIComponent(base)}&limit=${PAGE}&offset=${offset}`)
			.then((r) => (r.ok ? (r.json() as Promise<ApiListing>) : null))
			.then((j) => {
				// Ignore une page périmée (l'utilisateur a changé de dossier entre-temps).
				if (dirRef.current !== base) return;
				if (j?.files) {
					setFiles((f) => [
						...f,
						...j.files.map((x) => ({ id: x.path, name: x.name, kind: "file" as const, ext: x.ext })),
					]);
				}
			})
			.finally(() => {
				if (dirRef.current === base) setLoadingMore(false);
			});
	};

	const sorted = useMemo(() => {
		const ds = [...dirs].sort((a, b) => a.name.localeCompare(b.name, "fr", { numeric: true }));
		const fs = [...files].sort((a, b) => {
			if (sort === "type") {
				const e = (a.ext ?? "").localeCompare(b.ext ?? "");
				if (e !== 0) return e;
			}
			return a.name.localeCompare(b.name, "fr", { numeric: true });
		});
		return [...ds, ...fs];
	}, [dirs, files, sort]);

	if (loading) {
		return (
			<div className="flex items-center justify-center p-12">
				<div className="animate-spin rounded-full size-7 border-b-2 border-primary" />
			</div>
		);
	}
	if (sorted.length === 0) {
		return <p className="p-8 text-center text-sm text-on-surface-variant">Dossier vide.</p>;
	}

	// Galerie 3D : si le dossier courant est un parent de modèles (`…/_waza`, `…/_item`, …),
	// chaque sous-dossier est un code de modèle servi en GLB → aperçu 3D lazy par vignette.
	const modelSubMatch = dir.match(/(?:^|\/)_([a-z]+)$/);
	const modelSub =
		modelSubMatch && MODEL_PARENT_SUBS.has(modelSubMatch[1]) ? modelSubMatch[1] : null;
	const open = (it: Item) =>
		it.kind === "dir" ? onOpenDir(it.id) : onOpenFile(it.id, it.ext ?? "", it.name);
	const remaining = fileTotal - files.length;

	return (
		<div className="space-y-4">
			{view === "grid" ? (
				<div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 xl:grid-cols-5 gap-2">
					{sorted.map((it) => {
						const thumb = it.kind === "file" ? cpkThumbUrl(it.id, it.ext, 240) : null;
						return (
							<button
								key={it.id}
								type="button"
								onClick={() => open(it)}
								className="group flex flex-col rounded-xl bg-surface-container-low border border-outline-variant/15 overflow-hidden text-left hover:bg-surface-container transition-colors focus-visible:ring-2 focus-visible:ring-primary"
							>
								<div className="aspect-square flex items-center justify-center bg-surface-container/50 overflow-hidden">
									{thumb ? (
										// eslint-disable-next-line @next/next/no-img-element
										<img
											src={thumb}
											alt={it.name}
											loading="lazy"
											className="w-full h-full object-contain [image-rendering:auto]"
										/>
									) : modelSub && it.kind === "dir" ? (
										<CpkModelThumb
											glbUrl={`${CDN}/model-chr/${modelSub}/${it.name}.glb`}
											name={it.name}
										/>
									) : (
										<Icon
											name={itemIcon(it)}
											size={40}
											className={it.kind === "dir" ? "text-primary" : "text-on-surface-variant/60"}
										/>
									)}
								</div>
								<div className="p-2 min-w-0">
									<p className="truncate text-xs text-on-surface" title={it.name}>
										{it.name}
									</p>
									{it.kind === "dir" && typeof it.count === "number" && (
										<p className="text-[11px] text-on-surface-variant/70">
											{it.count.toLocaleString("fr")} fichiers
										</p>
									)}
								</div>
							</button>
						);
					})}
				</div>
			) : (
				<ul className="divide-y divide-outline-variant/10">
					{sorted.map((it) => (
						<li key={it.id}>
							<button
								type="button"
								onClick={() => open(it)}
								className="w-full flex items-center gap-3 h-12 px-2 rounded-lg text-left hover:bg-on-surface/[0.08] transition-colors"
							>
								<Icon
									name={itemIcon(it)}
									size={20}
									className={`shrink-0 ${it.kind === "dir" ? "text-primary" : "text-on-surface-variant"}`}
								/>
								<span className="truncate flex-1 text-sm text-on-surface">{it.name}</span>
								{it.kind === "dir" ? (
									<span className="text-xs text-on-surface-variant/70">
										{(it.count ?? 0).toLocaleString("fr")}
									</span>
								) : (
									<span className="text-xs font-mono text-on-surface-variant/60">.{it.ext}</span>
								)}
							</button>
						</li>
					))}
				</ul>
			)}

			{remaining > 0 && (
				<div className="text-center">
					<button
						type="button"
						onClick={loadMore}
						disabled={loadingMore}
						className="inline-flex items-center gap-2 h-9 px-4 rounded-full bg-surface-container-high text-on-surface text-sm hover:bg-surface-container-highest transition-colors disabled:opacity-60"
					>
						{loadingMore && <div className="animate-spin rounded-full size-4 border-b-2 border-primary" />}
						Afficher plus ({remaining.toLocaleString("fr")} restants)
					</button>
				</div>
			)}
		</div>
	);
}
