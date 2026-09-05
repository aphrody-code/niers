import "server-only";

/**
 * Énumération du corpus 3D — les codes de chaque famille, avec leur nom quand il en existe un.
 *
 * Deux sources, parce que les modèles ne se nomment pas de la même façon :
 *
 * - **Personnages** : le miroir SQLite. Eux seuls portent un nom affichable
 *   (`inagle_characters`), et leur code d'assemblage est le préfixe de `internal_code`
 *   (`c01000010_5000` → `c01000010`).
 * - **Le reste** : le VFS **live**, un sous-dossier par code sous `data/common/chr/_<famille>`.
 *   Aucun manifeste statique : l'ancienne galerie filtrait sur une liste écrite à la main qui
 *   plafonnait à 509 modèles sur les 6 236 du jeu.
 */

import { createSqliteClient } from "@rosegriffon/azalee/db";
import { lsOrNull } from "@rosegriffon/azalee/cpk/live";
import { type ModelFamily, modelFamily } from "@rosegriffon/azalee/cpk/models";

/** Un modèle du corpus. */
export interface ModelEntry {
	/** Code d'assemblage, ex. `c01000010` ou `a000010`. */
	code: string;
	/** Nom affichable, `null` si le jeu n'en donne pas (props, décors). */
	name: string | null;
	/** Slug de la fiche personnage, pour lier. `null` hors famille `perso`. */
	slug: string | null;
	/**
	 * Nombre de fichiers dans le dossier du modèle — maillage, squelette, motions, packs.
	 *
	 * C'est le seul contexte disponible à l'échelle d'une liste : il vient du listing du dossier
	 * PARENT, déjà chargé, et ne coûte donc aucune requête supplémentaire. `null` pour les
	 * personnages, dont les pièces sont éclatées entre corps, visage et uniforme.
	 */
	files: number | null;
}

/**
 * Filtre une liste de modèles sur une recherche libre — nom ET code.
 *
 * Comparaison insensible à la casse et aux accents : `deja` doit trouver « Déjà ». La recherche
 * porte sur la famille entière côté serveur, jamais sur la seule page rendue.
 */
export function filtrer(entrees: ModelEntry[], q: string): ModelEntry[] {
	const besoin = normaliser(q);
	if (!besoin) return entrees;
	return entrees.filter(
		(m) => normaliser(m.code).includes(besoin) || (m.name && normaliser(m.name).includes(besoin)),
	);
}

function normaliser(s: string): string {
	return s
		.normalize("NFD")
		.replace(/\p{Diacritic}/gu, "")
		.toLowerCase()
		.trim();
}

/** Ligne du miroir dont on a besoin. */
interface CharaRow {
	internal_code: string | null;
	name_fr: string | null;
	name_en: string | null;
	name_ja: string | null;
	slug: string | null;
}

let _characters: Promise<ModelEntry[]> | null = null;

/**
 * Les personnages, dédoublonnés par code d'assemblage et triés par nom.
 *
 * Un personnage a plusieurs variantes (`_5000`, `_5001`…) qui partagent le même code de modèle :
 * les lister toutes afficherait le même modèle des dizaines de fois. La promesse est mémorisée,
 * pas son résultat, pour que deux rendus concurrents partagent la requête.
 */
export function characters(): Promise<ModelEntry[]> {
	_characters ??= (async () => {
		const parCode = new Map<string, ModelEntry>();
		try {
			const client = createSqliteClient();
			const { data } = (await client
				.from("inagle_characters")
				.select("internal_code,name_fr,name_en,name_ja,slug")) as { data: CharaRow[] | null };
			for (const row of data ?? []) {
				const code = row.internal_code?.split("_")[0];
				if (!code) continue;
				if (parCode.has(code)) continue;
				parCode.set(code, {
					code,
					files: null,
					name: row.name_fr || row.name_en || row.name_ja || null,
					slug: row.slug,
				});
			}
		} catch {
			// Miroir absent : la famille rend vide plutôt que d'échouer.
		}
		return [...parCode.values()].toSorted((a, b) =>
			(a.name ?? a.code).localeCompare(b.name ?? b.code, "fr", { numeric: true }),
		);
	})();
	return _characters;
}

const _familles = new Map<ModelFamily, Promise<ModelEntry[]>>();

/**
 * Les codes d'une famille non-personnage, lus sur le VFS live.
 *
 * Un code = un sous-dossier de `data/common/chr/_<famille>`. Ces modèles n'ont pas de nom dans
 * les données du jeu : on rend le code, sans en inventer un.
 */
function famillleVfs(famille: ModelFamily): Promise<ModelEntry[]> {
	const cache = _familles.get(famille);
	if (cache) return cache;
	const p = (async () => {
		const def = modelFamily(famille);
		if (!def?.vfsDir) return [];
		const listing = await lsOrNull(def.vfsDir, 0);
		return (listing?.dirs ?? [])
			.map(
				(d) =>
					({ code: d.name, files: d.count ?? null, name: null, slug: null }) satisfies ModelEntry,
			)
			.toSorted((a, b) => a.code.localeCompare(b.code, "fr", { numeric: true }));
	})();
	_familles.set(famille, p);
	return p;
}

/** Tous les modèles d'une famille, quelle qu'en soit la source. */
export function modelesDe(famille: ModelFamily): Promise<ModelEntry[]> {
	return famille === "chara" ? characters() : famillleVfs(famille);
}
