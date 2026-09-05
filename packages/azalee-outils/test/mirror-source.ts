/**
 * La source de secours des contextes hors ligne : le miroir SQLite.
 *
 * `@rosegriffon/azalee` ne connaît plus SQLite depuis le lot J2 — son client a déménagé ici,
 * avec le reste de l'outillage. La bibliothèque n'a donc plus de source par défaut : c'est
 * l'hôte qui en fournit une, et c'est le but (sur Vercel, une lecture sans source doit lever,
 * pas rendre une page vide).
 *
 * Ce module la pose comme **défaut** et non comme fabrique explicite. La nuance est celle qui
 * réconcilie deux besoins opposés : `db.test.ts` vérifie qu'aucune fabrique n'est injectée au
 * départ (`hasDatabaseProvider() === false`), tandis que le serveur et le CLI doivent pouvoir
 * lire. Un défaut satisfait les deux — et il rend leur innocuité aux `afterEach` d'hygiène qui
 * remettent `setDatabaseProvider(null)`.
 *
 * Il n'injecte rien de lui-même : un module d'aide ne doit pas agir en étant simplement lu.
 */
import { setDefaultDatabaseProvider } from "@rosegriffon/azalee/db";
import type { SupabaseClient } from "@supabase/supabase-js";
import { createSqliteClient } from "../src/db/sqlite-client";

/**
 * Pose le miroir comme source de secours. La fabrique est paresseuse : `createSqliteClient()`
 * n'est appelé qu'à la première lecture réelle, si bien qu'un test qui ne touche pas aux
 * données ne réclame aucun miroir.
 */
export function poserMiroirParDefaut(): void {
	setDefaultDatabaseProvider(() => createSqliteClient() as unknown as SupabaseClient);
}
