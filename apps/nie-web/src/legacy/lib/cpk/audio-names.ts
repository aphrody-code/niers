import "server-only";

/**
 * Rattache une banque audio du jeu à ce qu'elle sonorise — le personnage, pour les banques de voix.
 *
 * Les banques de voix portent le code interne du personnage (`c01000010.acb`), et
 * `inagle_characters.internal_code` est de la forme `<code>_<variante>` (`c01000010_5000`) : la
 * jointure est donc un préfixe, pas une égalité.
 *
 * COUVERTURE MESURÉE (14 août 2026, VFS complet + miroir) : 1 923 des 4 539 banques japonaises
 * s'apparient à un personnage, soit 42 %. Les 2 616 restantes portent des codes absents du miroir
 * (PNJ, variantes non fichées). Elles restent listées et jouables, simplement sans nom — on
 * n'invente pas un porteur qu'on ne sait pas nommer.
 */

import { createSqliteClient } from "@rosegriffon/azalee/db";
import { voiceBankCharacterCode } from "@rosegriffon/azalee/cpk/audio";

/** Ce qu'une banque sonorise, quand on sait le dire. */
export interface BankOwner {
	/** Nom affichable (français, repli anglais puis japonais). */
	name: string;
	/** Slug de la fiche personnage, pour lier vers elle. */
	slug: string | null;
	/** Code interne complet, ex. `c01000010_5000`. */
	internalCode: string;
}

/** Ligne du miroir dont on a besoin. */
interface CharaRow {
	internal_code: string | null;
	name_fr: string | null;
	name_en: string | null;
	name_ja: string | null;
	slug: string | null;
}

let _index: Promise<Map<string, BankOwner>> | null = null;

/**
 * Index `code de banque → personnage`, construit une fois par processus.
 *
 * Une seule requête pour les 6 148 personnages, puis une Map : la page liste des milliers de
 * banques, et une requête par banque serait 4 539 allers-retours SQLite pour la même donnée.
 * La promesse elle-même est mémorisée, pas son résultat : deux rendus concurrents partagent la
 * même requête au lieu de la lancer deux fois.
 */
export function ownerIndex(): Promise<Map<string, BankOwner>> {
	_index ??= (async () => {
		const index = new Map<string, BankOwner>();
		try {
			const client = createSqliteClient();
			const { data } = (await client
				.from("inagle_characters")
				.select("internal_code,name_fr,name_en,name_ja,slug")) as { data: CharaRow[] | null };
			for (const row of data ?? []) {
				const code = row.internal_code?.split("_")[0]?.toLowerCase();
				if (!code) continue;
				const name = row.name_fr || row.name_en || row.name_ja;
				if (!name) continue;
				// Première variante rencontrée : les variantes d'un personnage partagent son nom.
				if (!index.has(code)) {
					index.set(code, { name, slug: row.slug, internalCode: row.internal_code ?? code });
				}
			}
		} catch {
			// Miroir absent (build fresh-checkout) : la galerie rend sans les noms, elle n'échoue pas.
		}
		return index;
	})();
	return _index;
}

/** Le personnage que sonorise une banque, ou `null` si ce n'est pas une banque de voix connue. */
export function bankOwner(index: Map<string, BankOwner>, path: string): BankOwner | null {
	const code = voiceBankCharacterCode(path);
	return code ? (index.get(code) ?? null) : null;
}
