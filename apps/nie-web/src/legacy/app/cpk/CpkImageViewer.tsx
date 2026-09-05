"use client";

/**
 * Viewer de texture `.g4tx` décodée **nativement dans le navigateur** (wasm `g4txToPng` —
 * DDS BC1/BC3/BC5/BC7 → RGBA → PNG, pleine résolution). Qualité : fond damier pour révéler
 * l'alpha, rendu net (`pixelated`) pour les petites textures, dimensions affichées, export PNG.
 * Repli sur le décodage CDN serveur (`/dx11`) si le wasm échoue.
 */
import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkRawUrl } from "@rosegriffon/azalee/cpk/shared";
import { g4txToPng } from "@/lib/cpk-wasm";

export interface CpkImageViewerProps {
	/** Chemin d'index du `.g4tx` (préfixe `data/`). */
	path: string;
	/** Nom du fichier (alt + nom de téléchargement). */
	name: string;
	/** URL CDN de repli (décodage serveur) si le wasm échoue. */
	fallbackUrl: string | null;
}

/** Damier de transparence (alpha visible) en data-URI, indépendant du thème. */
const CHECKER =
	"repeating-conic-gradient(rgba(120,120,120,.18) 0% 25%, transparent 0% 50%) 50% / 16px 16px";

export function CpkImageViewer({ path, name, fallbackUrl }: CpkImageViewerProps) {
	const [url, setUrl] = useState<string | null>(null);
	const [failed, setFailed] = useState(false);
	const [dim, setDim] = useState<{ w: number; h: number } | null>(null);

	useEffect(() => {
		let cancelled = false;
		let objectUrl: string | null = null;
		setUrl(null);
		setFailed(false);
		setDim(null);

		(async () => {
			try {
				const res = await fetch(cpkRawUrl(path));
				if (!res.ok) throw new Error(String(res.status));
				const bytes = new Uint8Array(await res.arrayBuffer());
				const png = await g4txToPng(bytes);
				if (cancelled) return;
				objectUrl = URL.createObjectURL(new Blob([png.slice()], { type: "image/png" }));
				setUrl(objectUrl);
			} catch {
				if (!cancelled) setFailed(true);
			}
		})();

		return () => {
			cancelled = true;
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	}, [path]);

	// Repli serveur (/dx11) si le décodage in-browser échoue.
	if (failed && fallbackUrl) {
		return (
			<a
				href={fallbackUrl}
				target="_blank"
				rel="noopener noreferrer"
				className="block"
				style={{ background: CHECKER }}
			>
				{/* eslint-disable-next-line @next/next/no-img-element */}
				<img
					src={fallbackUrl}
					alt={name}
					loading="lazy"
					className="mx-auto max-h-[60vh] w-auto object-contain p-4"
				/>
			</a>
		);
	}

	if (!url) {
		return (
			<div className="flex items-center justify-center p-10">
				<div className="animate-spin rounded-full size-7 border-b-2 border-primary" />
			</div>
		);
	}

	// Petites textures (icônes, sprites) : rendu net non lissé ; grandes : lissé.
	const crisp = dim !== null && Math.max(dim.w, dim.h) <= 256;

	return (
		<div>
			<a
				href={url}
				target="_blank"
				rel="noopener noreferrer"
				className="block"
				style={{ background: CHECKER }}
			>
				{/* PNG pleine résolution décodé in-browser (wasm). */}
				{/* eslint-disable-next-line @next/next/no-img-element */}
				<img
					src={url}
					alt={name}
					onLoad={(e) =>
						setDim({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })
					}
					className="mx-auto max-h-[60vh] w-auto object-contain p-4"
					style={{ imageRendering: crisp ? "pixelated" : "auto" }}
				/>
			</a>
			<div className="flex flex-wrap items-center justify-between gap-2 px-4 py-2 text-xs text-on-surface-variant border-t border-outline-variant/20">
				<span className="font-mono">
					{dim ? `${dim.w} × ${dim.h} px` : "…"} · PNG · décodé in-browser
				</span>
				<a
					href={url}
					download={`${name}.png`}
					className="inline-flex items-center gap-1 text-primary hover:underline"
				>
					<Icon name="download" size={14} /> PNG
				</a>
			</div>
		</div>
	);
}
