import crypto from "node:crypto";
import { createSupabaseServiceClient } from "@rosegriffon/db/service";
import bcrypt from "bcryptjs";
import { betterAuth } from "better-auth";
import { nextCookies } from "better-auth/next-js";
import { bearer } from "better-auth/plugins";
import { agentAuth } from "@better-auth/agent-auth";
import { Pool } from "pg";
import { logAudit } from "@/lib/audit";
import { ADMIN_ROLES } from "@/lib/auth-roles";
import { getIPIntelligence } from "@/lib/ip-intelligence";
import { getPublicOrigin } from "@/lib/site-url";
import { sendEmail } from "@/src/lib/email";
import {
	commonAuthOptions,
	createAccountOptions,
	deuxFacteurs,
	ensureUserProfile,
	googleProvider,
} from "@rosegriffon/auth";

export { ensureUserProfile } from "@rosegriffon/auth";

const getDatabaseURL = () => {
	const url = process.env.DATABASE_URL;
	if (!url || url.startsWith("eyJ2Ijo") || url === "undefined" || url === "null") {
		// Aucun repli, et surtout pas une chaine de connexion en dur.
		//
		// Il y en avait une ici : identifiants complets du Postgres de production, en clair dans
		// la source, utilisee des que la variable manquait ou arrivait sous forme de blob chiffre
		// Vercel. Deux consequences, et la seconde est la pire : le secret vit dans l'historique
		// git de tout clone du depot, et un deploiement mal configure se connectait quand meme —
		// donc sans jamais signaler qu'il lui manquait sa configuration.
		//
		// Mieux vaut un demarrage qui echoue et se lit qu'une authentification qui marche par
		// accident sur des identifiants que personne ne croit utiliser.
		throw new Error(
			"DATABASE_URL absente ou invalide. Better Auth ne peut pas ouvrir sa connexion " +
				"Postgres : renseigner la variable dans la configuration du deploiement.",
		);
	}
	return url;
};

const pool = new Pool({
	connectionString: getDatabaseURL(),
});

// Service-role Supabase client for hooks
const supabaseAdmin = createSupabaseServiceClient();

const getBetterAuthURL = () => {
	const url = process.env.BETTER_AUTH_URL;
	if (!url || url === "undefined" || url === "null" || !url.startsWith("http")) {
		// Origin public canonique (jamais le host interne 0.0.0.0:3003 du self-host).
		return getPublicOrigin();
	}
	return url;
};

function createAuth() {
	return betterAuth({
		database: pool,
		baseURL: getBetterAuthURL(),
		secret:
			process.env.BETTER_AUTH_SECRET ||
			"build-placeholder-secret-not-used-at-runtime-only-prevents-prerender-throw",

		...commonAuthOptions,

		// Le spread ci-dessus apporte la correspondance des colonnes ; il faut la
		// réétaler ici, sinon redéfinir `user` l'emporterait entièrement.
		// `createAccountOptions` active le changement d'adresse et la suppression
		// de compte, désactivés par défaut dans Better Auth : les boutons
		// correspondants de `/settings` échouaient donc systématiquement.
		user: {
			...commonAuthOptions.user,
			...createAccountOptions({
				appName: "Azalée",
				// Même pool que Better Auth : `profiles` et `auth.users` ne sont
				// atteignables que par SQL direct (cf. `beforeDelete`).
				executerSql: (requete, parametres) => pool.query(requete, parametres as unknown[]),
				sendEmail,
			}),
		},

		trustedOrigins: [
			getPublicOrigin(),
			"https://azalee.rosegriffon.fr",
			process.env.BETTER_AUTH_URL ?? "https://azalee.rosegriffon.fr",
			"http://localhost:3000",
			"http://localhost:3001",
		].filter((v, i, a) => v && a.indexOf(v) === i),

		emailVerification: {
			autoSignInAfterVerification: true,
			sendOnSignUp: true,
			sendVerificationEmail: async ({ user, url }) => {
				void sendEmail({
					to: user.email,
					subject: "Vérifie ton adresse email — Azalée",
					html: `<!DOCTYPE html>
<html lang="fr">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#1A120D;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#1A120D;padding:40px 20px">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#2D1F14;border-radius:28px;overflow:hidden">
        <tr><td style="padding:40px 32px 24px;text-align:center">
          <h1 style="margin:0 0 8px;font-size:22px;font-weight:800;color:#F2A93B">Confirme ton email</h1>
          <p style="margin:0;font-size:15px;color:#BBA998;line-height:1.5">
            Bienvenue sur Azalée ! Clique sur le bouton pour vérifier <strong style="color:#E8D5C4">${user.email}</strong>.
          </p>
        </td></tr>
        <tr><td style="padding:0 32px 32px;text-align:center">
          <a href="${url}" style="display:inline-block;padding:14px 36px;background:#8B5023;color:#fff;font-size:15px;font-weight:700;text-decoration:none;border-radius:100px">
            Vérifier mon email
          </a>
          <p style="margin:20px 0 0;font-size:12px;color:#6B5D52;line-height:1.5">
            Ce lien expire dans 1 heure. Si tu n'as pas fait cette demande, ignore cet email.
          </p>
        </td></tr>
        <tr><td style="padding:16px 32px;border-top:1px solid #3D2E22;text-align:center">
          <p style="margin:0;font-size:11px;color:#6B5D52">Azalée — Rose Griffon</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>`,
				});
			},
		},

		emailAndPassword: {
			enabled: true,
			password: {
				verify: async ({ hash, password }) => {
					if (hash.startsWith("$2a$") || hash.startsWith("$2b$")) {
						return bcrypt.compare(password, hash);
					}
					const { verifyPassword } = await import("better-auth/crypto");
					return verifyPassword({ hash, password });
				},
			},
			requireEmailVerification: false,
			sendResetPassword: async ({ user, url }) => {
				void sendEmail({
					to: user.email,
					subject: "Réinitialisation de ton mot de passe — Azalée",
					html: `<!DOCTYPE html>
<html lang="fr">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#1A120D;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#1A120D;padding:40px 20px">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#2D1F14;border-radius:28px;overflow:hidden">
        <tr><td style="padding:40px 32px 24px;text-align:center">
          <img src="${getPublicOrigin()}/logo.webp" alt="Azalée" width="48" height="48" style="border-radius:12px;margin-bottom:16px">
          <h1 style="margin:0 0 8px;font-size:22px;font-weight:800;color:#F2A93B">Mot de passe oublié ?</h1>
          <p style="margin:0;font-size:15px;color:#BBA998;line-height:1.5">
            Quelqu'un a demandé la réinitialisation du mot de passe pour <strong style="color:#E8D5C4">${user.email}</strong>.
          </p>
        </td></tr>
        <tr><td style="padding:0 32px 32px;text-align:center">
          <a href="${url}" style="display:inline-block;padding:14px 36px;background:#8B5023;color:#fff;font-size:15px;font-weight:700;text-decoration:none;border-radius:100px">
            Réinitialiser mon mot de passe
          </a>
          <p style="margin:20px 0 0;font-size:12px;color:#8A7A6D;line-height:1.5">
            Si le bouton ne fonctionne pas, copie ce lien :<br>
            <a href="${url}" style="color:#F2A93B;word-break:break-all">${url}</a>
          </p>
          <p style="margin:16px 0 0;font-size:12px;color:#6B5D52;line-height:1.5">
            Ce lien expire dans 1 heure. Si tu n'as pas fait cette demande, ignore cet email.
          </p>
        </td></tr>
        <tr><td style="padding:16px 32px;border-top:1px solid #3D2E22;text-align:center">
          <p style="margin:0;font-size:11px;color:#6B5D52">Azalée — Rose Griffon</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>`,
				});
			},
		},

		socialProviders: {
			discord: {
				clientId: process.env.DISCORD_CLIENT_ID!,
				clientSecret: process.env.DISCORD_CLIENT_SECRET!,
				// Better Auth force `prompt=none` par défaut sur Discord → tout user non
				// déjà autorisé est rejeté (consent_required) avant le callback. `consent`
				// affiche l'écran d'autorisation et marche pour tout le monde.
				prompt: "consent",
			},
			...googleProvider,
		},

		rateLimit: {
			...commonAuthOptions.rateLimit,
			customRules: {
				"/forget-password": { max: 3, window: 60 },
				"/sign-in/email": { max: 10, window: 60 },
				"/sign-up/email": { max: 5, window: 60 },
			},
		},

		account: {
			...commonAuthOptions.account,
			accountLinking: {
				enabled: true,
				trustedProviders: ["google", "discord"],
			},
		},

		advanced: {
			...commonAuthOptions.advanced,
			database: {
				generateId: () => crypto.randomUUID(),
			},
		},

		databaseHooks: {
			session: {
				create: {
					after: async (session) => {
						const ip = session.ipAddress || "unknown";
						const ipInfo = await getIPIntelligence(ip);

						await logAudit(
							session.userId,
							"login",
							{
								geo: ipInfo
									? {
											city: ipInfo.city,
											country: ipInfo.country,
											privacy: ipInfo.privacy,
											org: ipInfo.org,
										}
									: undefined,
							},
							{ ipAddress: ip }
						);
					},
					before: async (session) => {
						try {
							const { data: profile } = await supabaseAdmin
								.from("profiles")
								.select("role")
								.eq("id", session.userId)
								.maybeSingle();

							if (profile && ADMIN_ROLES.includes(profile.role as (typeof ADMIN_ROLES)[number])) {
								const tenYears = new Date();
								tenYears.setFullYear(tenYears.getFullYear() + 10);
								return { data: { ...session, expiresAt: tenYears } };
							}
						} catch {
							// ignore
						}
						return { data: session };
					},
				},
			},
			user: {
				create: {
					after: async (user, ctx) => {
						await ensureUserProfile(user, supabaseAdmin);

						const ip = ctx?.headers?.get("x-forwarded-for")?.split(",")[0]?.trim() || "unknown";
						const ipInfo = await getIPIntelligence(ip);

						await logAudit(
							user.id,
							"signup",
							{
								email: user.email,
								name: user.name,
								geo: ipInfo
									? {
											city: ipInfo.city,
											country: ipInfo.country,
											privacy: ipInfo.privacy,
										}
									: undefined,
							},
							{ ipAddress: ip }
						);
					},
				},
			},
		},

		plugins: [
			agentAuth({
				providerName: "Azalée",
				providerDescription: "Azalée project and wiki APIs for AI agents.",
				modes: ["delegated", "autonomous"],
				capabilities: [
					{
						name: "list_articles",
						description: "List news and wiki articles from Azalée.",
						input: {
							type: "object",
							properties: {
								limit: { type: "integer", minimum: 1, maximum: 100 },
							},
						},
					},
					{
						name: "get_article",
						description: "Get the content of a specific news or wiki article.",
						input: {
							type: "object",
							properties: {
								slug: { type: "string" },
							},
							required: ["slug"],
						},
					},
				],
				async onExecute({ capability, arguments: args }) {
					const supabase = createSupabaseServiceClient();

					switch (capability) {
						case "list_articles": {
							const limit = (args?.limit as number) || 20;
							const { data, error } = await supabase
								.from("articles")
								.select("id, title, slug, excerpt, status, published_at")
								.eq("app", "azalee")
								.eq("status", "published")
								.order("published_at", { ascending: false })
								.limit(limit);
							if (error) throw new Error(error.message);
							return data;
						}
						case "get_article": {
							const { data, error } = await supabase
								.from("articles")
								.select("id, title, slug, content_html, published_at")
								.eq("app", "azalee")
								.eq("slug", args?.slug as string)
								.maybeSingle();
							if (error) throw new Error(error.message);
							if (!data) throw new Error("Article not found");
							return data;
						}
						default:
							throw new Error(`Unsupported capability: ${capability}`);
					}
				},
			}),
			deuxFacteurs("Azalée"),
			bearer(),
			nextCookies(),
		],
	});
}

type Auth = ReturnType<typeof createAuth>;

function createAuthMock(): Auth {
	const noSession = async () => null;
	return {
		$ERROR_CODES: {} as never,
		$context: undefined as never,
		api: { getSession: noSession },
		handler: async () => new Response(null, { status: 503 }),
		options: {} as never,
	} as unknown as Auth;
}

let _auth: Auth;
try {
	_auth = createAuth();
} catch (error) {
	console.error(
		"[auth] betterAuth() init failed — falling back to mock:",
		error instanceof Error ? error.message : String(error)
	);
	_auth = createAuthMock();
}

export const auth: Auth = _auth;
