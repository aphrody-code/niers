/**
 * Types d'options des commandes `azalee`.
 *
 * Une interface **nommée et exportée** par commande : commander ne peut pas
 * inférer la forme d'un `.opts()`, ces types sont donc le seul contrat entre
 * la déclaration des drapeaux et le corps de l'action. Ils sont aussi ce que
 * consomme un appelant programmatique (tests, sidecar Tauri).
 *
 * Convention : `-j, --json` produit toujours un unique objet/tableau JSON sur
 * stdout, et rien d'autre.
 */

/** Socle commun : sortie JSON brute au lieu du rendu humain. */
export interface JsonOption {
	json?: boolean;
}

/** `azalee translate [text] [-j]` */
export type TranslateOptions = JsonOption;

/** `azalee search [query] [-j]` */
export type SearchOptions = JsonOption;

/** `azalee db [sql] [-s] [-j]` */
export interface DbOptions extends JsonOption {
	/** Interroge le miroir SQLite local au lieu de PostgreSQL. */
	sqlite?: boolean;
}

/** `azalee redis <cmd> <key> [val] [-j]` */
export type RedisOptions = JsonOption;

/** Sous-commandes acceptées par `azalee redis`. */
export type RedisSubcommand = "get" | "set" | "del";

/** `azalee glossary-rebuild [-j]` */
export type GlossaryRebuildOptions = JsonOption;

/** `azalee audit [-j]` */
export type AuditOptions = JsonOption;

/** `azalee rag [query] [-j] [-l <limit>]` */
export interface RagOptions extends JsonOption {
	/** Nombre maximum de passages renvoyés (chaîne : valeur brute commander). */
	limit: string;
}

/** `azalee status [-j]` */
export type StatusOptions = JsonOption;

/** `azalee wave [-c] [-l]` */
export interface WaveOptions {
	/** Runner de cycle complet (sync + build + restart) au lieu d'une passe. */
	cycle?: boolean;
	/** Boucle infinie, une vague toutes les 30 secondes. */
	loop?: boolean;
}

/** `azalee sync [-p] [-j]` */
export interface SyncOptions {
	/** Pousse les données inagle locales vers PostgreSQL. */
	push?: boolean;
	/** Rapatrie les corrections SQL vers `characters.json`. */
	json?: boolean;
}

/** `azalee compare <chara1> <chara2> [-l <level>] [-j]` */
export interface CompareOptions extends JsonOption {
	/** Niveau de comparaison, 1 à 99 (chaîne : valeur brute commander). */
	level: string;
}

/** `azalee chara [query] [-j]` */
export type CharaOptions = JsonOption;

/** `azalee dialogue [query] [-s <speaker>] [-j] [-l <limit>]` */
export interface DialogueOptions extends JsonOption {
	/** Filtre par locuteur (nom FR/EN/JA ou `charaId`). */
	speaker?: string;
	/** Nombre de répliques renvoyées (chaîne : valeur brute commander). */
	limit: string;
}

/** `azalee skill [query] [-j]` */
export type SkillOptions = JsonOption;

/** `azalee item [query] [-j]` */
export type ItemOptions = JsonOption;

/** `azalee team [query] [-j]` */
export type TeamOptions = JsonOption;

/** `azalee random-team [-f <formation>] [-e <element>] [-p <playstyle>] [-j]` */
export interface RandomTeamOptions extends JsonOption {
	/** Disposition demandée (`4-4-2`, `4-3-3`…). */
	formation: string;
	/** Élément privilégié (FR ou EN). */
	element?: string;
	/** Style de jeu privilégié (FR ou EN). */
	playstyle?: string;
}

/** Actions acceptées par `azalee team-builder`. */
export type TeamBuilderAction = "list" | "show" | "delete" | "save" | "generate";

/** `azalee team-builder <action> [args...] [-j] [-f <formation>]` */
export interface TeamBuilderOptions extends JsonOption {
	/** Formation imposée à l'action `generate`. */
	formation?: string;
}

/** `azalee test [-p]` */
export interface TestOptions {
	/** Délègue la suite à Playwright (E2E) au lieu du runner natif. */
	playwright?: boolean;
}

/** `azalee data migrate [--apply]` */
export interface DataMigrateOptions {
	/** Exécute réellement les migrations (sinon : liste en dry-run). */
	apply?: boolean;
}

/** `azalee data sync [--full] [--deletes]` */
export interface DataSyncOptions {
	/** Resynchronisation complète au lieu d'incrémentale. */
	full?: boolean;
	/** Propage aussi les suppressions. */
	deletes?: boolean;
}

/** `azalee serve [-p <port>] [-H <host>] [--cors <origin>] [-j]` */
export interface ServeOptions extends JsonOption {
	/** Port d'écoute (chaîne : valeur brute commander). */
	port: string;
	/** Interface d'écoute. */
	host: string;
	/** Origine CORS autorisée. */
	cors?: string;
}
