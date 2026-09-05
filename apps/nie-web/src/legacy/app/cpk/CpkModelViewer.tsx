"use client";

/**
 * Viewer de modèle 3D `.g4md`/`.g4mg` **assemblé nativement dans le navigateur** : récupère
 * la paire G4MD + G4MG (octets bruts via `/raw`) et l'assemble en GLB via wasm (`modelToGlb`,
 * mêmes parseurs Rust que le jeu), affiché dans `<model-viewer>` via un blob URL.
 */
import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkRawUrl } from "@rosegriffon/azalee/cpk/shared";
import { modelToGlb } from "@/lib/cpk-wasm";

const CharacterModelViewer = dynamic(() => import("@/components/wiki/CharacterModelViewer"), {
	ssr: false,
});

// Sous-domaines chr dont le G4MD est empaqueté dans le `.g4pkm` (pas en fichier libre) et
// que le serveur assemble en GLB **texturé** (route /model-chr/<sub>/<code>.glb).
const SERVER_SUBS = new Set(["waza", "item", "animal", "keshin", "armd"]);
const CDN = "https://cdn.rosegriffon.fr";

export function CpkModelViewer({ path, name }: { path: string; name: string }) {
	const [url, setUrl] = useState<string | null>(null);
	const [error, setError] = useState(false);
	const [source, setSource] = useState<"wasm" | "server">("wasm");

	useEffect(() => {
		let cancelled = false;
		let objectUrl: string | null = null;
		const controller = new AbortController();
		setUrl(null);
		setError(false);

		// Modèles à G4MD packé (_waza/_item/_keshin/_animal/_armd) → route serveur texturée.
		const sub = path.match(/\/common\/chr\/_([a-z]+)\/([^/]+)\/[^/]+\.g4m[dg]$/i);
		if (sub && SERVER_SUBS.has(sub[1].toLowerCase())) {
			setSource("server");
			setUrl(`${CDN}/model-chr/${sub[1].toLowerCase()}/${sub[2]}.glb`);
			return;
		}
		setSource("wasm");

		(async () => {
			try {
				// Paire G4MD (descripteur) + G4MG (géométrie) libres (cas _face/_uniform) →
				// assemblage in-browser (wasm).
				const mdPath = path.replace(/\.g4mg$/i, ".g4md");
				const mgPath = mdPath.replace(/\.g4md$/i, ".g4mg");
				const [mdRes, mgRes] = await Promise.all([
					fetch(cpkRawUrl(mdPath), { signal: controller.signal }),
					fetch(cpkRawUrl(mgPath), { signal: controller.signal }),
				]);
				if (!mdRes.ok || !mgRes.ok) throw new Error("paire g4md/g4mg introuvable");
				const md = new Uint8Array(await mdRes.arrayBuffer());
				const mg = new Uint8Array(await mgRes.arrayBuffer());
				if (cancelled) return;
				const glb = await modelToGlb(md, mg);
				if (cancelled) return;
				objectUrl = URL.createObjectURL(new Blob([glb.slice()], { type: "model/gltf-binary" }));
				setUrl(objectUrl);
			} catch {
				if (!cancelled) setError(true);
			}
		})();

		return () => {
			cancelled = true;
			controller.abort();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	}, [path]);

	if (error) {
		return (
			<div className="flex flex-col items-center gap-2 p-8 text-center">
				<Icon name="deployed_code" size={32} className="text-on-surface-variant/50" />
				<p className="text-sm text-on-surface-variant">
					Modèle non assemblable in-browser (paire G4MD/G4MG manquante ou incompatible).
				</p>
			</div>
		);
	}
	if (!url) {
		return (
			<div className="flex items-center justify-center p-10">
				<div className="animate-spin rounded-full size-7 border-b-2 border-primary" />
			</div>
		);
	}

	return (
		<div className="flex flex-col items-center gap-2 p-4">
			<p className="text-xs text-on-surface-variant">
				{source === "wasm"
					? "Géométrie assemblée dans le navigateur (G4MD + G4MG → GLB, WASM)"
					: "Modèle texturé assemblé par NieModelServer"}
			</p>
			<CharacterModelViewer glbUrl={url} name={name} inline />
			<a
				href={url}
				download={`${name}.glb`}
				className="text-xs text-primary hover:underline inline-flex items-center gap-1"
			>
				<Icon name="download" size={14} /> Télécharger le GLB
			</a>
		</div>
	);
}
