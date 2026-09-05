/**
 * Fournisseur de client base de données — point d'injection unique de la lib.
 *
 * L'hôte injecte le client qu'il veut : `apps/azalee` fournit son client
 * Supabase, l'outillage hors ligne fournit le client du miroir SQLite
 * (`@niers/azalee-tools`).
 *
 * ## Pourquoi il n'y a plus de défaut (lot J2, 2026-09-05)
 *
 * La bibliothèque retombait d'elle-même sur le miroir SQLite quand personne
 * n'injectait rien. Ce défaut était invisible et dangereux : sur Vercel le
 * fichier n'existe pas, et l'erreur ne survenait qu'au moment d'une requête, à
 * l'intérieur d'une page — donc sous la forme d'une page vide plutôt que d'un
 * démarrage refusé. Un hôte doit maintenant DIRE d'où viennent ses données.
 */

import type { SupabaseClient } from "@supabase/supabase-js";

/** Fabrique de client, synchrone ou asynchrone. */
export type DatabaseClientFactory = () => SupabaseClient | Promise<SupabaseClient>;

let factory: DatabaseClientFactory | null = null;
let defaut: DatabaseClientFactory | null = null;

/**
 * Pose la source de SECOURS, celle qui s'applique quand aucune fabrique explicite n'est
 * injectée.
 *
 * Elle n'existe que si un hôte la fournit : `@niers/azalee-tools` y met le miroir SQLite,
 * parce qu'une CLI ou une suite hors ligne lit légitimement un fichier local. Le wiki
 * serverless, lui, n'en pose aucune — sans injection explicite, ses lectures lèvent, ce qui
 * est le but du lot J2.
 *
 * La distinction compte : `setDatabaseProvider(null)` veut dire « retire MON client », pas
 * « supprime toute source ». Sans ce second niveau, les `afterEach` d'hygiène qui remettent
 * `null` — un usage parfaitement sain — condamnaient tous les fichiers de test suivants.
 */
export function setDefaultDatabaseProvider(next: DatabaseClientFactory | null): void {
	defaut = next;
}

/**
 * Injecte la fabrique de client utilisée par toute la lib. Passer `null` la retire et rend la
 * main à la source de secours, s'il y en a une.
 */
export function setDatabaseProvider(next: DatabaseClientFactory | null): void {
	factory = next;
}

/** Indique si une fabrique a été injectée par l'hôte. */
export function hasDatabaseProvider(): boolean {
	return factory !== null;
}

/**
 * Renvoie le client de lecture des données de jeu. Les modules `wiki/*`
 * n'utilisent que la surface `.from(table).select(...)`, commune au client
 * Supabase et au client miroir.
 */
export async function createClient(): Promise<SupabaseClient> {
	const choisie = factory ?? defaut;
	if (!choisie) {
		throw new Error(
			"Aucun client de données injecté : appelez setDatabaseProvider() avant toute lecture. " +
				"Le wiki injecte son client Supabase (apps/azalee/lib/azalee-runtime), " +
				"l'outillage hors ligne celui du miroir (@niers/azalee-tools).",
		);
	}
	return await choisie();
}
