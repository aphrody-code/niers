/**
 * `WebSource` — l'implémentation d'`AssetSource` pour Aphrody, dans un navigateur.
 *
 * Elle ne parle qu'à `nie-site` (`crates/tools/nie-site`), en HTTP. Chaque route employée ici
 * est vérifiée par la suite de bout en bout `scripts/e2e-site.sh` : aucune n'est supposée, et
 * la forme des réponses est copiée des structures Rust, pas devinée. Un DTO cité de mémoire
 * rend `undefined` en silence — c'est le mode d'échec le plus coûteux d'un client HTTP.
 *
 * Ce que cet hôte ne sait pas faire, il le dit par `capacites()` plutôt que de le laisser
 * échouer au clic : pas d'écriture, pas de disque, pas de Lua, pas de Blender.
 */
import {
	type AssetSource,
	AUCUNE_CAPACITE,
	type CapacitesSource,
	capacitesDepuisServeur,
	type ContenuDossier,
	type EntreeVfs,
	type OptionsPage,
} from "./contract";
import {
	cheminAudio,
	cheminFilm,
	cheminModeleComplet,
	cheminTexture,
} from "./url-conventions";
import {
	catalogue as catalogueSite,
	type Page,
	type SanteApi,
	sante as santeSite,
	urlDossier,
	urlFichier as urlFichierSite,
	type VueCatalogue,
} from "./nie-site";

/** Réglages de la source web. L'origine vide vise le serveur qui a servi la page. */
export interface OptionsWebSource {
	/** Origine de `nie-site`. Vide en production : le bundle et l'API partagent l'origine. */
	origine?: string;
}

async function lire<T>(url: string, signal?: AbortSignal): Promise<T> {
	const r = await fetch(url, { signal, headers: { accept: "application/json" } });
	if (!r.ok) throw new Error(`${url} a répondu ${r.status}`);
	return (await r.json()) as T;
}

/**
 * Construit la source web.
 *
 * `capacites()` interroge le serveur au lieu de les affirmer : le VFS s'indexe en tâche de
 * fond, et le miroir peut être absent. Une capacité annoncée sans mesure serait un mensonge
 * dont l'interface hériterait.
 */
export function creerWebSource({ origine = "" }: OptionsWebSource = {}): AssetSource {
	const abs = (chemin: string) => `${origine}${chemin}`;

	return {
		hote: "aphrody",

		async capacites(): Promise<CapacitesSource> {
			try {
				const s = await santeSite();
				return capacitesDepuisServeur(s.capacites);
			} catch {
				// Serveur injoignable : on ne sait rien faire, et on le dit. L'interface
				// affichera son état dégradé au lieu de multiplier les requêtes vouées à échouer.
				return { ...AUCUNE_CAPACITE };
			}
		},

		sante(signal?: AbortSignal): Promise<SanteApi> {
			return santeSite(signal);
		},

		async parcourir(prefixe: string, signal?: AbortSignal): Promise<ContenuDossier> {
			const brut = await lire<{ dossiers?: string[]; fichiers?: EntreeVfs[] }>(
				abs(urlDossier(prefixe)),
				signal,
			);
			// Le serveur omet une clé vide plutôt que de rendre un tableau nul ; on normalise
			// ici pour que l'appelant n'ait jamais à tester l'absence.
			return { prefixe, dossiers: brut.dossiers ?? [], fichiers: brut.fichiers ?? [] };
		},

		catalogue(vue: VueCatalogue, options: OptionsPage = {}): Promise<Page<EntreeVfs>> {
			return catalogueSite(vue, options);
		},

		urlFichier(cheminVfs: string): string {
			return abs(urlFichierSite(cheminVfs));
		},

		// Les vues décodées passent par le proxy `/assets`, qui relaie le chemin VERBATIM à
		// `nie-model-serve`. Ces URL doivent donc suivre les conventions de l'amont, pas une
		// convention plausible — et elles ont des pièges relevés à l'usage : une texture
		// s'adresse SANS son `.g4tx` (`.g4tx.png` rend 404), un modèle complet vit sous
		// `/model-full/`, et l'identifiant d'un AWB se passe en `?id=`, pas `?awb=`.
		//
		// D'où le branchement sur `@niers/catalog/jeu` plutôt qu'une reconstruction : ces 69
		// fonctions portent les règles réelles. Écrites de tête, trois de ces quatre URL
		// étaient fausses, et un 404 sur ces routes ne se rattache jamais spontanément à
		// l'URL — on cherche le décodage.
		urlTexture: (cheminVfs: string) => abs(`/assets${cheminTexture(cheminVfs)}`),
		urlModele: (code: string) => abs(`/assets${cheminModeleComplet(code)}`),
		urlAudio: (cheminVfs: string, awbId?: number | null) =>
			abs(`/assets${cheminAudio(cheminVfs, awbId)}`),
		urlVideo: (cheminVfs: string) => abs(`/assets${cheminFilm(cheminVfs)}`),

		// Le redimensionnement est fait par l'amont (`?w=`), pas par le navigateur : télécharger
		// une texture de plusieurs mégaoctets pour l'afficher en 96 px gâche la bande passante
		// de l'utilisateur et la mémoire de l'onglet.
		async vignette(cheminVfs: string, cote: number): Promise<string | null> {
			return `${abs(`/assets${cheminTexture(cheminVfs)}`)}?w=${cote}`;
		},

		wiki<T>(table: string, options: OptionsPage = {}): Promise<Page<T>> {
			const { page = 1, parPage = 60, signal } = options;
			return lire<Page<T>>(abs(`/api/v1/${table}?page=${page}&per_page=${parPage}`), signal);
		},
	};
}
