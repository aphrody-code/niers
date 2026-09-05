/**
 * Navigateur de l'index CPK — route catch-all qui MIROITE l'arbre des fichiers
 * IEVR (250 800 entrées, source Redis db3 `iev:file:index`, matérialisée en
 * SQLite via `lib/cpk/index.ts`).
 *
 *   /cpk                         → racine (common/ + dx11/, avec comptes).
 *   /cpk/common                  → sous-dossiers de common (event, text, chr…).
 *   /cpk/common/chr/_face/...    → descend l'arbre.
 *   /cpk/dx11/menu/.../c.g4tx    → fiche fichier (preview selon le type).
 *
 * Server Component (lit le SQLite miroir, `import "server-only"` côté lib) ; les
 * composants interactifs (recherche, viewer 3D) sont des îlots `"use client"`
 * qui n'importent que le `-shared` client-safe. `force-dynamic` (catch-all, pas
 * de prerender de 250k chemins).
 *
 * Le path alias `@/*` mappe `./src/*` ET la racine `./*` ; les libs vivent à
 * `lib/cpk/*` (racine).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { CpkExplorer } from "@/app/cpk/CpkExplorer";
import { fileMeta, listDirPaged } from "@/lib/cpk/index";
import { normalizeDir, segLabel } from "@rosegriffon/azalee/cpk/shared";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/**
 * Reconstitue le chemin d'index (`data/...`) depuis les segments d'URL.
 *
 * Les segments arrivent DÉJÀ décodés : Next décode chaque segment d'un catch-all dans
 * `getRouteMatcher` (`shared/lib/router/utils/route-matcher.js`). Redécoder ici était donc
 * un second décodage, qui cassait deux fois : `/cpk/data/100%25` levait une `URIError` sur
 * `decodeURIComponent("100%")` (page d'erreur), et `/cpk/data/dx11/menu%252Fmisc` devenait
 * silencieusement le chemin `dx11/menu/misc` au lieu du fichier littéral `menu%2Fmisc`.
 */
function indexPathFromSegments(segments: string[] | undefined): string {
	const rel = (segments ?? []).join("/");
	if (rel === "" || rel === "data") return "data";
	return rel.startsWith("data/") ? rel : `data/${rel}`;
}

export async function generateMetadata({
	params,
}: {
	params: Promise<{ path?: string[] }>;
}): Promise<Metadata> {
	const { path } = await params;
	const indexPath = indexPathFromSegments(path);
	const rel = indexPath.replace(/^data\/?/, "");
	const parts = rel.split("/").filter(Boolean);
	const last = parts[parts.length - 1];
	const title = last ? segLabel(last) : "Fichiers CPK";
	// Le chemin d'index est décodé : ré-encoder segment par segment pour que la canonique
	// reste une URL valide même sur un nom de fichier contenant `%`, `#` ou une espace.
	const canonicalPath = parts.map((s) => encodeURIComponent(s)).join("/");
	return {
		alternates: { canonical: `/cpk${canonicalPath ? `/${canonicalPath}` : ""}` },
		description:
			"Navigateur complet des archives CPK d'Inazuma Eleven: Victory Road — textures, modèles 3D, sons, vidéos et données de jeu, décodés à la volée par le CDN.",
		title: `${title} | Fichiers CPK · Inazuma Eleven Victory Road - Azalée`,
	};
}

export default async function CpkPage({
	params,
}: {
	params: Promise<{ path?: string[] }>;
}) {
	const { path } = await params;
	const indexPath = indexPathFromSegments(path);

	if (indexPath.includes("..")) {
		notFound();
	}

	// Un chemin peut être un FICHIER (feuille de l'index) ou un RÉPERTOIRE.
	const meta = indexPath === "data" ? null : fileMeta(indexPath);
	const segments = indexPath.replace(/^data\/?/, "").split("/").filter(Boolean);

	// Validation : fichier (meta) OU répertoire peuplé OU racine → explorateur ; sinon 404.
	if (!meta && indexPath !== "data") {
		const listing = listDirPaged(indexPath, 1, 0);
		if (listing.dirs.length === 0 && listing.fileTotal === 0) {
			notFound();
		}
	}

	const isRoot = normalizeDir(indexPath) === "data";

	return (
		<div className="space-y-4">
			<header className="space-y-1">
				<h1 className="text-fluid-headline-md font-extrabold text-on-surface">
					{isRoot ? "Fichiers du jeu (CPK)" : segLabel(segments[segments.length - 1] ?? "")}
				</h1>
				<p className="text-sm text-on-surface-variant max-w-2xl">
					Explorateur des 250 800 fichiers d'Inazuma Eleven: Victory Road — arbre virtualisé,
					décodage à la volée (texture, modèle 3D, audio, vidéo, données).
				</p>
			</header>

			<CpkExplorer initialPath={indexPath} />
		</div>
	);
}
