"use client";

/**
 * Fiche d'un fichier de l'index CPK : preview selon le type + métadonnées.
 *
 * Le CONTENU décodé est servi par le CDN (`cdn.rosegriffon.fr`) — ce composant
 * ne fait que référencer la bonne URL CDN selon la famille de preview :
 *   - `image`  → `<img>` (variante WebP via CDN, lien PNG plein).
 *   - `model`  → viewer 3D `<model-viewer>` (`/model-full/<code>.glb`) si le code
 *                est un perso/keshin/uniforme, sinon lien `/model/<code>.glb`.
 *   - `sound`  → balise `<audio>` (le CDN sert le flux décodé).
 *   - `movie`  → balise `<video>`.
 *   - `text`   → fetch + rendu `<pre>` (best-effort, le CDN sert le brut).
 *   - `config` / `raw` → métadonnées + lien de téléchargement CDN.
 *
 * ⚠ `"use client"` : importe UNIQUEMENT le `-shared` client-safe (types + URLs),
 * jamais la lib serveur `lib/cpk/index.ts` (sqlite/Node → casse le bundle).
 */
import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { CpkAudioViewer } from "@/app/cpk/CpkAudioViewer";
import { CpkConfigViewer } from "@/app/cpk/CpkConfigViewer";
import { CpkHexViewer } from "@/app/cpk/CpkHexViewer";
import { CpkImageViewer } from "@/app/cpk/CpkImageViewer";
import { CpkModelViewer } from "@/app/cpk/CpkModelViewer";
import { CpkPackageViewer } from "@/app/cpk/CpkPackageViewer";
import {
	cpkPreviewKind,
	cpkRawUrl,
	cpkVideoUrl,
	previewKindLabel,
	type CpkPreviewKind,
} from "@rosegriffon/azalee/cpk/shared";

export interface CpkFilePreviewProps {
	name: string;
	ext: string;
	path: string;
	/** Archive CPK source (métadonnée d'affichage ; absente quand ouvert depuis l'arbre). */
	cpk?: string;
	/** Famille de contenu CDN brute (image|model|raw) issue de la lib serveur. */
	kind: "image" | "model" | "raw";
	/** URL CDN du contenu décodé (ou null). */
	assetUrl: string | null;
	/** URL vignette WebP (images uniquement, ou null). */
	thumbUrl?: string | null;
}

export function CpkFilePreview(props: CpkFilePreviewProps) {
	const { name, ext, path, cpk, assetUrl } = props;
	const previewKind: CpkPreviewKind = cpkPreviewKind(ext);

	return (
		<div className="space-y-6">
			<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low overflow-hidden">
				<PreviewBody {...props} previewKind={previewKind} />
			</div>

			{/* Métadonnées du fichier */}
			<dl className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
				<MetaRow label="Type" value={previewKindLabel(previewKind)} />
				<MetaRow label="Extension" value={`.${ext}`} mono />
				<MetaRow label="Nom" value={name} mono />
				{cpk && <MetaRow label="Archive CPK" value={cpk} mono truncate />}
				<MetaRow label="Chemin" value={path} mono truncate full />
			</dl>

			{/* Liens CDN. Le lien « contenu décodé » n'est montré que pour les images : la branche
			    image de `cpkAssetUrl` (g4tx → /dx11/<…>.png) est vérifiée 200, alors que la branche
			    model (/model-full/<code>.glb) 404 pour les codes non assemblables (uniforme/visage —
			    le viewer 3D fait l'assemblage in-browser + propose son propre GLB), et la branche raw
			    pointe sur /raw, identique au bouton « Télécharger l'original » ci-dessous. */}
			<div className="flex flex-wrap gap-2">
				{assetUrl && props.kind === "image" && (
					<a
						href={assetUrl}
						target="_blank"
						rel="noopener noreferrer"
						className="inline-flex items-center gap-2 h-11 sm:h-10 px-4 rounded-full bg-primary text-on-primary text-sm font-semibold hover:opacity-90 transition"
					>
						<Icon name="open_in_new" size={16} />
						PNG décodé (CDN)
					</a>
				)}
				<a
					href={cpkRawUrl(path)}
					download
					className="inline-flex items-center gap-2 h-11 sm:h-10 px-4 rounded-full border border-outline-variant/40 bg-surface-container-low text-on-surface text-sm font-semibold hover:border-primary transition"
				>
					<Icon name="download" size={16} />
					Télécharger l'original
				</a>
			</div>
		</div>
	);
}

function PreviewBody(props: CpkFilePreviewProps & { previewKind: CpkPreviewKind }) {
	const { name, path, assetUrl, previewKind } = props;

	if (previewKind === "image") {
		// Décodage g4tx→PNG NATIF IN-BROWSER (wasm) depuis /raw ; repli serveur /dx11.
		return <CpkImageViewer path={path} name={name} fallbackUrl={assetUrl} />;
	}

	if (previewKind === "model") {
		// Aperçu du modèle de CE fichier : assemblage in-browser de sa paire g4md+g4mg (wasm).
		return <CpkModelViewer path={path} name={name} />;
	}

	if (previewKind === "sound") {
		// Audio CRI (acb/awb/hca/adx) décodé NATIVEMENT in-browser (wasm) ; repli serveur.
		return <CpkAudioViewer path={path} />;
	}

	if (previewKind === "movie") {
		// Vidéo USM (CRI Sofdec2) démuxée + remuxée en MP4 H.264 par nie-model-serve
		// (`/video/`) → lisible nativement. `preload="metadata"` : décodage au clic.
		return (
			<div className="flex flex-col items-center gap-3 p-6 text-center">
				<p className="text-xs text-on-surface-variant">
					Vidéo USM décodée en direct (Sofdec2 → MP4 H.264)
				</p>
				{/* eslint-disable-next-line jsx-a11y/media-has-caption */}
				<video
					controls
					preload="metadata"
					className="w-full max-w-2xl rounded-lg"
					src={cpkVideoUrl(path)}
				/>
			</div>
		);
	}

	if (previewKind === "text") {
		return <TextPreview url={cpkRawUrl(path)} />;
	}

	if (previewKind === "config") {
		return <CpkConfigViewer path={path} />;
	}

	if (previewKind === "package") {
		// Archive G4PK (~45k fichiers) : liste des sous-fichiers décodée in-browser (wasm).
		return <CpkPackageViewer path={path} />;
	}

	// raw : dump hexadécimal des premiers Ko (offset | hex | ASCII).
	return <CpkHexViewer url={cpkRawUrl(path)} />;
}

/** Preview texte best-effort : fetch le contenu brut et l'affiche en `<pre>`. */
function TextPreview({ url }: { url: string }) {
	const [content, setContent] = useState<string | null>(null);
	const [error, setError] = useState(false);

	useEffect(() => {
		let cancelled = false;
		setContent(null);
		setError(false);
		fetch(url)
			.then((r) => (r.ok ? r.text() : Promise.reject(new Error(String(r.status)))))
			.then((t) => {
				if (!cancelled) setContent(t.slice(0, 50_000));
			})
			.catch(() => {
				if (!cancelled) setError(true);
			});
		return () => {
			cancelled = true;
		};
	}, [url]);

	if (error) {
		return (
			<div className="p-8 text-center text-sm text-on-surface-variant">
				Contenu texte indisponible.
			</div>
		);
	}
	if (content === null) {
		return (
			<div className="flex items-center justify-center p-10">
				<div className="animate-spin rounded-full size-6 border-b-2 border-primary" />
			</div>
		);
	}
	return (
		<pre className="max-h-[60vh] overflow-auto p-4 text-xs text-on-surface font-mono whitespace-pre-wrap break-words">
			{content}
		</pre>
	);
}

function MetaRow({
	label,
	value,
	mono = false,
	truncate = false,
	full = false,
}: {
	label: string;
	value: string;
	mono?: boolean;
	truncate?: boolean;
	full?: boolean;
}) {
	return (
		<div
			className={`rounded-xl border border-outline-variant/30 bg-surface-container-low px-3 py-2 ${full ? "sm:col-span-2" : ""}`}
		>
			<dt className="text-[10px] font-medium text-on-surface-variant uppercase tracking-wider">
				{label}
			</dt>
			<dd
				className={`mt-0.5 text-on-surface ${mono ? "font-mono text-xs" : ""} ${truncate ? "truncate" : "break-words"}`}
				title={value}
			>
				{value}
			</dd>
		</div>
	);
}
