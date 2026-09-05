import { createClient as createSupabaseClient, type SupabaseClient } from "@supabase/supabase-js";
import { headers } from "next/headers";
import { auth } from "@/lib/auth";
import { mintSupabaseJwt } from "@/lib/supabase/jwt";
import { cleAnonSupabase, origineSupabase } from "@/lib/supabase/url";
import type { Database } from "@rosegriffon/db";

// Résolution unique, partagée par les quatre modules du chemin de requête : cf. `./url`.
// Le jeton anonyme en dur qui servait de repli ici a disparu avec la cascade — un secret
// écrit dans la source est une fuite, même quand il est public.
const supabaseUrl = origineSupabase();
const supabaseAnonKey = cleAnonSupabase();

// Skip the Better Auth -> Supabase JWT bridge when the local SUPABASE_JWT_SECRET
// Is desynchronized from Supabase Cloud (minted JWTs rejected with PGRST301).
// Lectures publiques via anon + GRANT SELECT sur schema public (appliqué 2026-04-20).
// Défaut: true. Pour réactiver le bridge après avoir resyncé le secret: DISABLE_SUPABASE_JWT_BRIDGE=false.
const DISABLE_SUPABASE_JWT_BRIDGE = process.env.DISABLE_SUPABASE_JWT_BRIDGE !== "false";

/**
 * Rend un client Supabase serveur. Si une session Better Auth existe ET que le pont JWT est
 * actif, un jeton est frappé pour qu'`auth.uid()` fonctionne dans les RLS ; sinon le client
 * est anonyme.
 *
 * ## Ce qui a été retiré ici, et pourquoi (lot J2, 2026-09-05)
 *
 * Le client était enveloppé dans un `Proxy` qui détournait `from("inagle_*")` vers un cache
 * SQLite local, avec repli Postgres quand le pilote manquait. Cette bifurcation n'a plus
 * de sens en serverless : le fichier n'existe pas sur Vercel, le repli était donc pris à
 * *chaque* requête — au prix d'un `try/catch` sur un message d'erreur, ce qui est une
 * détection fragile. Surtout, elle faisait exister deux sources de vérité pour les mêmes
 * tables : c'est cette dualité qui a produit le faux vert du 2026-09-05.
 *
 * Le cache local reste disponible pour l'outillage hors ligne, dans
 * `@niers/azalee-tools` — jamais dans le chemin d'une page.
 */
export const createClient = async (): Promise<SupabaseClient> => {
	let accessToken: string | undefined;

	if (!DISABLE_SUPABASE_JWT_BRIDGE) {
		try {
			const reqHeaders = await headers();
			const session = await auth.api.getSession({ headers: reqHeaders });

			if (session?.user?.id) {
				accessToken = mintSupabaseJwt(session.user.id);
			}
		} catch {
			// No session or headers not available — use anon client
		}
	}

	let pgClient: SupabaseClient;
	if (accessToken) {
		pgClient = createSupabaseClient<Database>(supabaseUrl, supabaseAnonKey, {
			global: {
				headers: {
					Authorization: `Bearer ${accessToken}`,
				},
			},
		}) as unknown as SupabaseClient;
	} else {
		pgClient = createSupabaseClient<Database>(
			supabaseUrl,
			supabaseAnonKey
		) as unknown as SupabaseClient;
	}

	return pgClient;
};
