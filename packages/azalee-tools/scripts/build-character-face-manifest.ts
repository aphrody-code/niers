#!/usr/bin/env bun
/**
 * Génère `data/character-face-manifest.json` : la liste EXHAUSTIVE des codes de base
 * de personnage `c<NNNNNNNN>` qui possèdent réellement un visage variant-1 dans le
 * dump dx11, d'après `inagle_game_assets` (table-of-truth des URL servies par le CDN,
 * 40471 lignes — RLS anon SELECT en place).
 *
 * `getCharacterFaceUrl` construit toujours l'URL `icon_chr/face/{base}_l_{base}_1_l00.png`
 * (variant 1). Sans gate, tout perso sans face dans le dump déclenche un 404 silencieux
 * remplacé par le placeholder côté <Image onError> — ce qui masque le problème et
 * gaspille une requête. Ce manifest permet à `getCharacterFaceUrl` de renvoyer
 * directement `PLACEHOLDERS.character` quand le code est absent (azalee-3, plan §3 #10).
 *
 * Source : `inagle_game_assets.path ~ '^menu/icon_chr/face/<base>_l_<base>_1_l00.png$'`.
 * Aligné sur le format réel des chemins (cf. azalee/src/lib/images/utils.ts).
 *
 * Run : `bun scripts/build-character-face-manifest.ts`
 */
import path from "node:path";

/** Données figées du package (`packages/azalee/src/data`). */
const DATA_DIR = path.resolve(import.meta.dir, "../src/data");

const SUPABASE_URL = process.env.NEXT_PUBLIC_SUPABASE_URL;
const SUPABASE_KEY = process.env.SUPABASE_SERVICE_ROLE_KEY;

if (!SUPABASE_URL || !SUPABASE_KEY) {
	console.error("Missing NEXT_PUBLIC_SUPABASE_URL or SUPABASE_SERVICE_ROLE_KEY");
	process.exit(1);
}

// Récupère uniquement les faces variant-1 (celles que getCharacterFaceUrl peut servir).
const PATTERN = "menu/icon_chr/face/%_1_l00.png";
const bases = new Set<string>();
const PAGE = 1000;

for (let offset = 0; ; offset += PAGE) {
	const url =
		`${SUPABASE_URL}/rest/v1/inagle_game_assets` +
		`?select=path&path=like.${encodeURIComponent(PATTERN)}` +
		`&limit=${PAGE}&offset=${offset}`;
	const r = await fetch(url, {
		headers: {
			apikey: SUPABASE_KEY,
			authorization: `Bearer ${SUPABASE_KEY}`,
		},
	});
	if (!r.ok) {
		console.error(`Supabase fetch failed: ${r.status} ${await r.text()}`);
		process.exit(1);
	}
	const rows = (await r.json()) as Array<{ path: string }>;
	if (rows.length === 0) {
		break;
	}
	for (const { path } of rows) {
		// menu/icon_chr/face/<base>_l_<base>_1_l00.png → <base>
		const m = path.match(/^menu\/icon_chr\/face\/([^/]+?)_l_\1_1_l00\.png$/);
		if (m) {
			bases.add(m[1]);
		}
	}
	if (rows.length < PAGE) {
		break;
	}
}

const sorted = [...bases].sort();
const out = path.join(DATA_DIR, "character-face-manifest.json");
await Bun.write(out, JSON.stringify(sorted, null, 0));

console.log(`Face base codes with variant-1: ${sorted.length}`);
console.log(`Written: ${out}`);
