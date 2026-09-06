/**
 * Passerelle de types pour `@aphrody/bxc/privacy`.
 *
 * ## Pourquoi ce fichier existe
 *
 * `@aphrody/bxc` publie ses SOURCES TypeScript et declare `"types": "./src/api/browser.ts"`.
 * Quand `tsc` compile, depuis `@rosegriffon/cron`, les sources de `@aphrody/ietv` (un paquet du
 * workspace, donc traverse), il ne parvient pas a lire le sous-chemin `./privacy` comme source
 * de types : il retombe sur le champ `types` du paquet et rend
 * `TS2305: Module '"@aphrody/bxc"' has no exported member 'detectPii'` — en citant la RACINE,
 * alors que le code importe bien `"@aphrody/bxc/privacy"`.
 *
 * Ce que la mesure a etabli, dans l'ordre :
 *
 * 1. le paquet 0.9.0 EST installe (linker `isolated` : il vit dans le `node_modules` de chaque
 *    paquet qui le declare, pas a la racine — chercher `node_modules/@aphrody` a la racine
 *    donne « absent » et fait conclure a tort a une install manquante) ;
 * 2. `@aphrody/bxc/privacy` exporte bien les quatre symboles (`detectPii`, `redactPii`,
 *    `redactObject`, `PiiMatch`) — verifie dans `src/privacy/index.ts` ;
 * 3. `packages/ietv` se typecheck SEUL sans erreur : le defaut n'apparait qu'a la traversee ;
 * 4. declarer `@aphrody/bxc` dans les dependances de `cron` etait necessaire (le linker isole
 *    ne l'exposait pas) mais pas suffisant ;
 * 5. un mapping `paths` vers les sources du paquet ne change rien — ce n'est pas la resolution
 *    du CHEMIN qui echoue, c'est la lecture des types du sous-chemin.
 *
 * Ce fichier disparait le jour ou `@aphrody/bxc` publie des `.d.ts` (ou un `typesVersions`).
 * Il ne change rien a l'execution : Bun, lui, resout le sous-chemin sans probleme.
 */

declare module "@aphrody/bxc/privacy" {
	/** Une occurrence de donnee personnelle reperee dans un texte. */
	export interface PiiMatch {
		kind: string;
		value: string;
		start: number;
		end: number;
		[cle: string]: unknown;
	}

	/** Repere les donnees personnelles d'un texte. */
	export function detectPii(text: string, opts?: Record<string, unknown>): PiiMatch[];

	/** Masque les donnees personnelles d'un texte. */
	export function redactPii(
		text: string,
		opts?: Record<string, unknown>,
	): { text: string; matches: PiiMatch[]; [cle: string]: unknown };

	/** Masque les donnees personnelles d'une structure, en preservant sa forme. */
	export function redactObject<T>(value: T, opts?: Record<string, unknown>): T;
}
