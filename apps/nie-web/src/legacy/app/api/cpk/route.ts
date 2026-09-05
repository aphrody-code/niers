/**
 * API REST de l'index CPK — navigateur d'arbre des fichiers IEVR.
 *
 *   GET /api/cpk?path=<dir>[&offset=&limit=] → listing paginé JSON
 *                                              { dir, dirs[], files[], fileTotal, fileOffset }
 *   GET /api/cpk?path=<file>&meta=1          → métadonnées du fichier + URL CDN du contenu
 *
 * Source : `lib/cpk/index.ts` (SQLite matérialisé depuis l'index Redis db3,
 * `import "server-only"`). Le CONTENU décodé d'un fichier est servi par le CDN
 * (`assetUrl`/`thumbUrl`) ; cette API ne renvoie que la structure + les URLs.
 *
 * Le listing est **paginé côté serveur** (`listDirPaged`) : les sous-dossiers sont
 * toujours complets, mais les fichiers directs sont tranchés (`limit`/`offset`).
 * Indispensable pour les répertoires massifs (`common/sound/ja` ≈ 15 k fichiers
 * directs) — sans ça l'arbre/la grille téléchargeaient le répertoire entier d'un coup.
 *
 * `runtime = "nodejs"` (bun:sqlite, pas Edge). Listing immutable côté client
 * (l'index ne change qu'à la régénération de l'artefact) → cache court.
 */
import { NextResponse } from "next/server";
import { fileMeta, listDirPaged, searchFiles } from "@/lib/cpk/index";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

/** Page de fichiers par défaut / plafond (borne le payload JSON d'un répertoire). */
const DEFAULT_LIMIT = 1000;
const MAX_LIMIT = 5000;

/** Parse un entier de query borné à `[min, max]`, repli `fallback`. */
function clampInt(raw: string | null, fallback: number, min: number, max: number): number {
	const n = Number.parseInt(raw ?? "", 10);
	if (!Number.isFinite(n)) return fallback;
	return Math.min(max, Math.max(min, n));
}

export function GET(request: Request): NextResponse {
	const { searchParams } = new URL(request.url);
	const rawPath = searchParams.get("path") ?? "";
	const wantMeta = searchParams.get("meta") === "1";
	const query = searchParams.get("q");
	const offset = clampInt(searchParams.get("offset"), 0, 0, Number.MAX_SAFE_INTEGER);
	const limit = clampInt(searchParams.get("limit"), DEFAULT_LIMIT, 1, MAX_LIMIT);

	// Garde-fou : pas de `..`, on borne au sous-arbre `data/` de l'index.
	if (rawPath.includes("..")) {
		return NextResponse.json({ error: "invalid path" }, { status: 400 });
	}

	try {
		if (query != null) {
			const files = searchFiles(query, 200);
			return NextResponse.json(
				{ query, files, count: files.length },
				{ headers: { "Cache-Control": "public, max-age=60" } },
			);
		}
		if (wantMeta) {
			const meta = fileMeta(rawPath);
			if (!meta) {
				return NextResponse.json({ error: "not found", path: rawPath }, { status: 404 });
			}
			return NextResponse.json(meta, {
				headers: { "Cache-Control": "public, max-age=300, s-maxage=300" },
			});
		}

		const listing = listDirPaged(rawPath, limit, offset);
		return NextResponse.json(listing, {
			headers: { "Cache-Control": "public, max-age=300, s-maxage=300" },
		});
	} catch (error) {
		const message = error instanceof Error ? error.message : "internal error";
		return NextResponse.json({ error: message }, { status: 500 });
	}
}
