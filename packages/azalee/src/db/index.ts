/**
 * Injection du client de données.
 *
 * Ce module ne connaît plus aucune base : il ne fait que transporter la fabrique que l'hôte
 * lui donne (`setDatabaseProvider`). Le wiki y injecte son client Supabase.
 *
 * Le client SQLite et la résolution du miroir ont quitté cette porte au lot J2 — ils vivent
 * dans `@niers/azalee-tools`, hors du chemin d'une page. Tant qu'ils étaient exportés ici,
 * une page pouvait les atteindre par mégarde et lire un fichier qui n'existe pas en
 * serverless : la panne se voyait alors sous forme de page vide, jamais d'erreur.
 */

export {
	createClient,
	hasDatabaseProvider,
	setDatabaseProvider,
	setDefaultDatabaseProvider,
	type DatabaseClientFactory,
} from "./provider";

// Porte UNIQUE. `./db/*` a été retiré des `exports` du paquet le 2026-09-05 : tant qu'il
// existait, `@rosegriffon/azalee/db` et `@rosegriffon/azalee/db/provider` désignaient le même
// fichier par deux chemins, et le linker isolé de Bun en chargeait DEUX instances — chacune
// avec sa propre fabrique. Le préchargement de test injectait dans l'une, le serveur lisait
// l'autre : 13 tests échouaient en annonçant « aucun client injecté » juste après une
// injection réussie. Un état global n'a de sens que derrière un seul chemin d'import.
