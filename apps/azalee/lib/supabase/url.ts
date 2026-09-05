/**
 * La règle unique de résolution de l'origine Supabase et de sa clé anonyme.
 *
 * ## Pourquoi ce module existe
 *
 * La même cascade était recopiée dans quatre modules du chemin de requête
 * (`lib/supabase/server.ts`, `lib/supabase/public.ts`, `lib/api-client.ts`,
 * `src/lib/api/supabase.ts`), chacun essayant l'URL INTERNE du VPS **avant** l'URL
 * publique, puis retombant sur `http://127.0.0.1:8811`. Corriger un site n'en corrigeait
 * aucun autre : c'est ce qui a produit le faux vert du 2026-09-05, où `/chara` rendait un
 * HTTP 200 de 136 921 octets contenant **zéro** lien de personnage.
 *
 * ## Deux décisions, et leur raison
 *
 * 1. **L'URL interne du VPS n'est plus lue.** Elle désignait un proxy `127.0.0.1` local.
 *    Depuis une fonction serverless, cette adresse existe — c'est la fonction elle-même — et
 *    la requête échoue ou rend du vide *sans erreur*. Une variable qui ne peut pas être juste
 *    en production n'a pas à être consultée en production.
 * 2. **Aucun repli silencieux en production.** Sans URL, le module lève. Un repli aurait rendu
 *    un site qui répond 200 à tout et n'affiche rien — le mode de panne le plus coûteux du
 *    projet, parce qu'aucune sonde d'état ne le voit. En développement, le repli local reste,
 *    parce qu'il y est vrai.
 */

const REPLI_DEV = "http://127.0.0.1:8811";

/** Une origine exploitable : une URL http(s), et pas un secret collé par erreur. */
function origineValide(v: string | undefined | null): v is string {
	return Boolean(v && v !== "undefined" && v !== "null" && /^https?:\/\//.test(v) && !v.startsWith("eyJ2Ijo"));
}

/** Une clé exploitable : ni un objet JSON, ni un blob chiffré Vercel. */
function cleValide(v: string | undefined | null): v is string {
	return Boolean(v && v !== "undefined" && v !== "null" && !v.startsWith("{") && !v.startsWith("eyJ2Ijo"));
}

function enProduction(): boolean {
	return process.env.NODE_ENV === "production";
}

/**
 * L'origine Supabase. `NEXT_PUBLIC_SUPABASE_URL` d'abord, son alias `…_PUBLISHABLE_URL`
 * ensuite — les deux désignent le **même** projet Cloud, ce n'est donc pas une cascade entre
 * deux sources mais deux noms d'une seule.
 *
 * @throws si aucune n'est posée en production : mieux vaut un déploiement qui échoue qu'un
 * site qui répond 200 sur des pages vides.
 */
export function origineSupabase(): string {
	const candidats = [
		process.env.NEXT_PUBLIC_SUPABASE_URL,
		process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_URL,
	];
	const trouvee = candidats.find(origineValide);
	if (trouvee) {
		return trouvee;
	}
	if (enProduction()) {
		throw new Error(
			"NEXT_PUBLIC_SUPABASE_URL absente ou invalide. Refus de démarrer sur un repli local : " +
				"le site répondrait 200 avec des pages vides."
		);
	}
	return REPLI_DEV;
}

/**
 * La clé anonyme. Même règle : deux noms d'une seule clé, et aucun jeton en dur.
 *
 * @throws si aucune n'est posée en production.
 */
export function cleAnonSupabase(): string {
	const candidats = [
		process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY,
		process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY,
	];
	const trouvee = candidats.find(cleValide);
	if (trouvee) {
		return trouvee;
	}
	if (enProduction()) {
		throw new Error("NEXT_PUBLIC_SUPABASE_ANON_KEY absente ou invalide.");
	}
	return "";
}
