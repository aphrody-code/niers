import "server-only";

/**
 * Nommer un fichier média autrement que par son identifiant.
 *
 * `ev01_00050`, `c01000010`, `icon_item01` ne disent rien à un lecteur. L'ordre de résolution
 * est donc : **un vrai nom** issu des données du jeu, sinon **le contexte** (épisode, langue,
 * rôle du dossier), et **le code en dernier** — relégué sous le titre, jamais à sa place.
 *
 * Rien n'est deviné. Un libellé n'est produit que s'il repose sur une donnée vérifiée : la table
 * `inagle_events` du miroir pour les cinématiques d'épisode, le suffixe de langue des écrans-titre
 * (attesté par les neuf variantes de chaque fichier). Sans preuve, le nom du fichier reste le
 * titre — mieux vaut un code affiché qu'un libellé inventé.
 */

import { createSqliteClient } from "@rosegriffon/azalee/db";

/** Un libellé résolu : ce qu'on affiche, et dans quel ordre. */
export interface MediaLabel {
	/** Titre affiché. Jamais vide. */
	title: string;
	/** Contexte (épisode, langue, famille), `null` s'il n'y en a pas d'attesté. */
	context: string | null;
	/** Le code d'origine, à reléguer sous le titre. `null` s'il EST le titre. */
	code: string | null;
}

/** Langues des écrans-titre, telles que les fichiers les nomment. */
const LANGUES: Record<string, string> = {
	JP: "japonais",
	EN: "anglais",
	fr: "français",
	de: "allemand",
	es: "espagnol",
	it: "italien",
	pt: "portugais",
	CN: "chinois simplifié",
	TW: "chinois traditionnel",
};

let _episodes: Promise<Map<string, string>> | null = null;

/**
 * `event_id → épisode`, depuis `inagle_events`.
 *
 * 56 des 96 cinématiques du jeu portent un `event_id` présent dans cette table (mesuré) : c'est
 * la seule source qui rattache un film à un moment de l'histoire.
 */
function episodes(): Promise<Map<string, string>> {
	_episodes ??= (async () => {
		const index = new Map<string, string>();
		try {
			const client = createSqliteClient();
			const { data } = (await client.from("inagle_events").select("event_id,episode")) as {
				data: { event_id: string | null; episode: string | null }[] | null;
			};
			for (const row of data ?? []) {
				if (row.event_id && row.episode) index.set(row.event_id, row.episode);
			}
		} catch {
			// Miroir absent : on retombe sur le contexte structurel, pas sur une erreur.
		}
		return index;
	})();
	return _episodes;
}

/**
 * Libellé d'une cinématique, d'après son nom de fichier et la table des événements.
 *
 * @param nom nom du `.usm` sans extension, ex. `ev01_00050` ou `Chronicle_Title_JP_01`.
 */
export async function videoLabel(nom: string): Promise<MediaLabel> {
	// Écrans-titre : le suffixe de langue est attesté par les neuf variantes de chaque fichier.
	const titre = /^(Chronicle|NIE)_Title_([A-Za-z]+)_(\d+)$/.exec(nom);
	if (titre) {
		const [, famille, lang, num] = titre;
		const langue = LANGUES[lang ?? ""] ?? lang;
		return {
			title: famille === "Chronicle" ? `Écran-titre — Chroniques ${num}` : `Écran-titre ${num}`,
			context: langue ? `en ${langue}` : null,
			code: nom,
		};
	}

	// Cinématique d'événement : l'épisode vient du miroir, jamais d'une déduction sur le numéro.
	if (/^ev\d/i.test(nom)) {
		const episode = (await episodes()).get(nom);
		return {
			title: episode ? `Cinématique — épisode ${episode}` : "Cinématique d'événement",
			context: null,
			code: nom,
		};
	}

	// Rien d'attesté : le nom du fichier EST le titre. On n'invente pas de libellé.
	return { title: nom, context: null, code: null };
}

let _itemsParTexture: Promise<Map<string, string>> | null = null;

/**
 * `internal_code d'objet → nom`, qui sert à nommer les textures d'icônes.
 *
 * Les noms de textures des conteneurs d'icônes sont les codes internes des objets — constaté :
 * `icon_item01.g4tx` porte `performance_type_01`…`performance_type_17`, et `inagle_items`
 * contient exactement ces codes. C'est ce qui transforme une grille de codes en grille d'objets.
 */
export function itemNames(): Promise<Map<string, string>> {
	_itemsParTexture ??= (async () => {
		const index = new Map<string, string>();
		try {
			const client = createSqliteClient();
			const { data } = (await client
				.from("inagle_items")
				.select("internal_code,name_fr,name_en,name_ja")) as {
				data:
					| {
							internal_code: string | null;
							name_fr: string | null;
							name_en: string | null;
							name_ja: string | null;
					  }[]
					| null;
			};
			for (const row of data ?? []) {
				const nom = row.name_fr || row.name_en || row.name_ja;
				if (row.internal_code && nom) index.set(row.internal_code.toLowerCase(), nom);
			}
		} catch {
			// Miroir absent : les textures gardent leur nom technique.
		}
		return index;
	})();
	return _itemsParTexture;
}
